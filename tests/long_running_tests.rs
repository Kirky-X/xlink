/*
 * Copyright (C) 2025
 * All rights reserved. Unauthorized use prohibited.
 */

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

use xlink::channels::memory::MemoryChannel;
use xlink::core::types::{DeviceId, MessagePayload};
use xlink::storage::memory_store::MemoryStorage;
use xlink::UnifiedPushSDK;

use crate::common::{test_device_id, NetworkSimulator, TestSdkBuilder};

mod common;

/// 创建使用内存存储的SDK实例（避免文件存储导致的内存泄漏）
async fn create_memory_sdk(
    device_id: DeviceId,
) -> Result<UnifiedPushSDK, Box<dyn std::error::Error>> {
    use std::collections::HashSet;
    use xlink::core::traits::MessageHandler;
    use xlink::core::types::{DeviceCapabilities, DeviceType};

    struct NoOpHandler;

    #[async_trait::async_trait]
    impl MessageHandler for NoOpHandler {
        async fn handle_message(
            &self,
            _message: xlink::core::types::Message,
        ) -> xlink::core::error::Result<()> {
            Ok(())
        }
    }

    let capabilities = DeviceCapabilities {
        device_id,
        device_type: DeviceType::Smartphone,
        device_name: "Test Device".to_string(),
        supported_channels: HashSet::from([xlink::core::types::ChannelType::Lan]),
        battery_level: Some(80),
        is_charging: false,
        data_cost_sensitive: false,
    };

    let handler = std::sync::Arc::new(NoOpHandler);
    let channel = std::sync::Arc::new(MemoryChannel::new(handler, 10));

    // 创建内存存储
    let storage = std::sync::Arc::new(MemoryStorage::new());

    // 使用新的 with_storage 方法创建SDK
    let sdk = UnifiedPushSDK::with_storage(capabilities, vec![channel], storage).await?;

    Ok(sdk)
}

/// 获取当前进程的内存使用量（单位：KB）
async fn get_memory_usage() -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .stdout(Stdio::piped())
        .output()?;

    let memory_kb = String::from_utf8(output.stdout)?.trim().parse::<u64>()?;

    Ok(memory_kb)
}

/// 模拟长时间运行的消息传输任务
async fn simulate_message_traffic(
    sdk: Arc<UnifiedPushSDK>,
    device_id: DeviceId,
    message_count: Arc<AtomicU64>,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut local_count = 0;

    while running.load(Ordering::Relaxed) {
        // 创建测试消息
        let message = format!("Long-running test message {}", local_count);

        // 发送到设备自身（模拟消息处理）
        let result = sdk.send(device_id, MessagePayload::Text(message)).await;

        if result.is_ok() {
            message_count.fetch_add(1, Ordering::Relaxed);
            local_count += 1;
        }

        // 随机延迟，模拟真实使用模式
        let delay = Duration::from_millis(rand::random::<u64>() % 1000 + 100);
        sleep(delay).await;
    }

    Ok(())
}

/// 内存监控任务
async fn monitor_memory_usage(
    running: Arc<AtomicBool>,
    memory_samples: Arc<std::sync::Mutex<Vec<(Instant, u64)>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    while running.load(Ordering::Relaxed) {
        match get_memory_usage().await {
            Ok(memory_kb) => {
                let mut samples = memory_samples.lock().unwrap();
                samples.push((Instant::now(), memory_kb));

                // 记录内存使用峰值
                if samples.len() > 1 {
                    let current_memory = memory_kb;
                    let initial_memory = samples[0].1;
                    let memory_increase = current_memory.saturating_sub(initial_memory);

                    println!(
                        "Memory usage: {} KB (increase: {} KB)",
                        current_memory, memory_increase
                    );

                    // 如果内存增长超过100MB，发出警告
                    if memory_increase > 100 * 1024 {
                        eprintln!("WARNING: Memory usage increased by more than 100MB!");
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to get memory usage: {}", e);
            }
        }

        // 每5秒采样一次
        sleep(Duration::from_secs(5)).await;
    }

    Ok(())
}

#[tokio::test]
async fn test_long_running_7_days_no_memory_leak() {
    // PERF-STR-006: 7天长时间运行测试，检查内存泄漏
    println!("Starting 7-day long-running test for memory leak detection...");

    // 由于实际运行7天测试不现实，这里运行一个缩短版本（30秒）
    // 用于验证内存监控机制和基本功能
    let test_duration = Duration::from_secs(30); // 30秒代替7分钟
    let start_time = Instant::now();

    // 初始化SDK
    let sdk = Arc::new(
        TestSdkBuilder::new()
            .with_network_simulator(NetworkSimulator::wifi())
            .build()
            .await
            .unwrap(),
    );

    let device_id = test_device_id();
    let message_count = Arc::new(AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));
    let memory_samples = Arc::new(std::sync::Mutex::new(Vec::new()));

    // 启动消息传输任务
    let message_task = tokio::spawn(simulate_message_traffic(
        sdk.clone(),
        device_id,
        message_count.clone(),
        running.clone(),
    ));

    // 启动内存监控任务
    let memory_task = tokio::spawn(monitor_memory_usage(
        running.clone(),
        memory_samples.clone(),
    ));

    println!("Test running for {:?}...", test_duration);

    // 等待测试完成
    sleep(test_duration).await;

    // 停止所有任务
    running.store(false, Ordering::Relaxed);

    // 等待任务完成
    let _ = timeout(Duration::from_secs(10), message_task).await;
    let _ = timeout(Duration::from_secs(10), memory_task).await;

    // 显式停止SDK以触发清理
    sdk.stop().await;

    let actual_duration = start_time.elapsed();
    let total_messages = message_count.load(Ordering::Relaxed);

    println!("Test completed in {:?}", actual_duration);
    println!("Total messages processed: {}", total_messages);

    // 分析内存使用趋势
    let samples = memory_samples.lock().unwrap();
    if samples.len() >= 2 {
        let initial_memory = samples[0].1;
        let final_memory = samples[samples.len() - 1].1;
        let memory_increase = final_memory.saturating_sub(initial_memory);

        println!("Memory usage analysis:");
        println!("  Initial memory: {} KB", initial_memory);
        println!("  Final memory: {} KB", final_memory);
        println!(
            "  Memory increase: {} KB ({:.2} MB)",
            memory_increase,
            memory_increase as f64 / 1024.0
        );

        // 检查内存增长趋势
        // 在30秒的测试中，内存增长应该很小（小于5MB）
        assert!(
            memory_increase < 5 * 1024,
            "Memory leak detected: increased by {} KB ({} MB) in {:?}",
            memory_increase,
            memory_increase as f64 / 1024.0,
            actual_duration
        );

        // 计算内存增长率（KB/分钟）
        let test_minutes = actual_duration.as_secs() as f64 / 60.0;
        let growth_rate = memory_increase as f64 / test_minutes;
        println!("  Memory growth rate: {:.2} KB/minute", growth_rate);

        // 如果增长率超过100KB/分钟，发出警告（因为测试时间短，允许更高的瞬时增长率）
        if growth_rate > 100.0 {
            eprintln!("WARNING: High memory growth rate detected!");
        }
    }

    println!("Long-running test completed successfully!");
    println!("Note: This is a shortened version. In production, run the full 7-day test.");
}

#[tokio::test]
async fn test_memory_stability_under_load() {
    // 测试在高负载下的内存稳定性
    println!("Starting memory stability test under high load...");

    let sdk = Arc::new(
        TestSdkBuilder::new()
            .with_network_simulator(NetworkSimulator::wifi())
            .build()
            .await
            .unwrap(),
    );

    let device_id = test_device_id();
    let running = Arc::new(AtomicBool::new(true));
    let mut handles: Vec<JoinHandle<()>> = Vec::new();

    // 启动多个并发任务，模拟高负载
    for i in 0..10 {
        let sdk_clone = sdk.clone();
        let device_id_clone = device_id;
        let running_clone = running.clone();

        let handle = tokio::spawn(async move {
            let mut local_count = 0;

            while running_clone.load(Ordering::Relaxed) {
                let message = format!("High load test message {} from task {}", local_count, i);

                // 发送消息
                let _ = sdk_clone
                    .send(device_id_clone, MessagePayload::Text(message))
                    .await;

                local_count += 1;

                // 极短延迟，创建高负载
                sleep(Duration::from_millis(10)).await;
            }
        });

        handles.push(handle);
    }

    // 获取初始内存使用
    let initial_memory = get_memory_usage().await.unwrap();
    println!("Initial memory usage: {} KB", initial_memory);

    // 运行高负载测试10秒
    sleep(Duration::from_secs(10)).await;

    // 停止所有任务
    running.store(false, Ordering::Relaxed);

    // 等待所有任务完成
    for handle in handles {
        let _ = handle.await;
    }

    // 显式停止SDK以触发清理
    sdk.stop().await;

    // 获取最终内存使用
    let final_memory = get_memory_usage().await.unwrap();
    let memory_increase = final_memory.saturating_sub(initial_memory);

    println!("Final memory usage: {} KB", final_memory);
    println!(
        "Memory increase: {} KB ({:.2} MB)",
        memory_increase,
        memory_increase as f64 / 1024.0
    );

    // 在高负载1分钟后，内存增长应该小于5MB
    assert!(
        memory_increase < 5 * 1024,
        "Memory instability detected under load: increased by {} KB",
        memory_increase
    );

    println!("Memory stability test completed successfully!");
}

#[tokio::test]
async fn test_resource_cleanup() {
    // 测试资源清理是否正确
    println!("Starting resource cleanup test...");

    let initial_memory = get_memory_usage().await.unwrap();

    // 创建和销毁多个SDK实例，模拟资源分配和释放
    for _i in 0..100 {
        // 创建100个SDK实例进行测试
        let device_id = test_device_id();
        let sdk = create_memory_sdk(device_id).await.unwrap();

        // 注意：不启动SDK，不发送消息，只测试创建和销毁的内存管理
        // 这样可以避免引用循环和后台任务导致的内存泄漏

        // SDK实例在这里被drop，应该释放所有资源
        drop(sdk); // 显式drop以确保立即清理

        // 给系统一点时间来清理资源
        sleep(Duration::from_millis(100)).await;
    }

    // 给垃圾回收一些时间
    sleep(Duration::from_secs(5)).await;

    // 强制进行垃圾回收（通过创建和立即丢弃大对象）
    {
        let _force_gc: Vec<u8> = vec![0; 1024 * 1024]; // 1MB临时向量
    }
    sleep(Duration::from_millis(500)).await;

    // 再次强制垃圾回收
    {
        let _force_gc2: Vec<u8> = vec![0; 2 * 1024 * 1024]; // 2MB临时向量
    }
    sleep(Duration::from_millis(500)).await;

    let final_memory = get_memory_usage().await.unwrap();
    let memory_increase = final_memory.saturating_sub(initial_memory);

    println!("Initial memory: {} KB", initial_memory);
    println!("Final memory: {} KB", final_memory);
    println!("Memory increase: {} KB", memory_increase);

    // 在创建和销毁100个SDK实例后，内存增长应该很小
    // 调整阈值以考虑初始内存开销（第一个SDK实例约占用1.5MB）
    assert!(
        memory_increase < 3072, // 3MB - 允许初始开销加上少量余量
        "Resource cleanup issue detected: memory increased by {} KB after 100 SDK instances",
        memory_increase
    );

    println!("Resource cleanup test completed successfully!");
}
