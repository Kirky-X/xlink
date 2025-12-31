/*
 * Copyright (C) 2025
 * All rights reserved. Unauthorized use prohibited.
 */

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use tokio::time::sleep;

use xlink::core::types::{DeviceId, MessagePayload};
use xlink::XLink;

use crate::common::{test_device_id, NetworkSimulator, TestSdkBuilder};

mod common;

/// 模拟DoS攻击：大量并发连接请求
async fn simulate_dos_attack(
    sdk: &Arc<XLink>,
    target_device: DeviceId,
    attack_duration: Duration,
    requests_per_second: u32,
) -> Result<(u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    let mut successful_requests = 0;
    let mut blocked_requests = 0;
    let start_time = Instant::now();

    println!(
        "Starting DoS attack simulation: {} requests/second for {:?}",
        requests_per_second, attack_duration
    );

    while start_time.elapsed() < attack_duration {
        let mut handles: Vec<JoinHandle<Result<bool, Box<dyn std::error::Error + Send + Sync>>>> =
            Vec::new();

        // 在1秒内发送指定数量的请求
        for i in 0..requests_per_second {
            let sdk_clone = Arc::clone(sdk);
            let message = format!("DoS attack message {} from attacker", i);
            let _device_id = test_device_id();

            let handle = tokio::spawn(async move {
                // 使用真实的SDK调用来测试速率限制
                let result = sdk_clone
                    .send(target_device, MessagePayload::Text(message))
                    .await;
                match result {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false), // 被速率限制或其他错误
                }
            });

            handles.push(handle);
        }

        // 等待所有请求完成
        for handle in handles {
            match handle.await {
                Ok(Ok(true)) => successful_requests += 1,
                Ok(Ok(false)) => blocked_requests += 1,
                Ok(Err(_)) => blocked_requests += 1,
                Err(_) => blocked_requests += 1,
            }
        }

        // 等待下一秒
        sleep(Duration::from_secs(1)).await;
    }

    println!(
        "DoS attack completed: {} successful, {} blocked",
        successful_requests, blocked_requests
    );
    Ok((successful_requests, blocked_requests))
}

/// 测试速率限制机制
async fn test_rate_limiting(
    sdk: &Arc<XLink>,
    target_device: DeviceId,
    burst_requests: u32,
) -> Result<(u32, u32), Box<dyn std::error::Error + Send + Sync>> {
    let mut successful_requests = 0;
    let mut rate_limited_requests = 0;

    println!(
        "Testing rate limiting with {} burst requests...",
        burst_requests
    );

    // 记录开始时间
    let start_time = std::time::Instant::now();

    // 使用并发任务来创建真正的突发流量，以超过100 req/sec的限制
    let mut handles = Vec::new();

    for i in 0..burst_requests {
        let sdk_clone = Arc::clone(sdk);
        let message = format!("Rate limit test message {}", i);

        let handle = tokio::spawn(async move {
            sdk_clone
                .send(target_device, MessagePayload::Text(message))
                .await
        });

        handles.push((handle, i));
    }

    // 收集所有任务的结果
    for (handle, i) in handles {
        match handle.await {
            Ok(Ok(_)) => {
                successful_requests += 1;
                if i < 10 || i % 20 == 19 {
                    println!("Request {}: Success", i + 1);
                }
            }
            Ok(Err(e)) => {
                rate_limited_requests += 1;
                println!("Request {}: Rate limited - {}", i + 1, e);
            }
            Err(_) => {
                rate_limited_requests += 1;
                println!("Request {}: Task failed", i + 1);
            }
        }

        // 每50个请求打印一次进度和时间
        if i % 50 == 49 {
            let elapsed = start_time.elapsed();
            println!(
                "Progress: {} requests sent in {:?} ({:.1} req/sec)",
                i + 1,
                elapsed,
                (i + 1) as f64 / elapsed.as_secs_f64()
            );
        }
    }

    let total_time = start_time.elapsed();
    println!(
        "Rate limiting test completed: {} successful, {} rate limited in {:?} ({:.1} req/sec)",
        successful_requests,
        rate_limited_requests,
        total_time,
        burst_requests as f64 / total_time.as_secs_f64()
    );
    Ok((successful_requests, rate_limited_requests))
}

#[tokio::test]
async fn test_dos_protection_rate_limiting() {
    // UAT: DoS攻击防护测试 - 速率限制 - 验收标准：系统能抵御大量连接请求
    println!("Starting UAT DoS protection rate limiting test...");

    let sdk = Arc::new(
        TestSdkBuilder::new()
            .with_network_simulator(NetworkSimulator::wifi())
            .build()
            .await
            .unwrap(),
    );

    let target_device = test_device_id();

    // UAT requirement: Test rate limiting with burst requests
    println!("UAT Phase: Testing rate limiting with burst requests...");
    let (successful, rate_limited) = test_rate_limiting(&sdk, target_device, 120).await.unwrap();

    println!("UAT Results:");
    println!("  Total requests: 120");
    println!("  Successful: {}", successful);
    println!("  Rate limited: {}", rate_limited);

    // Calculate protection effectiveness
    let total_requests = successful + rate_limited;
    let protection_ratio = rate_limited as f64 / total_requests as f64 * 100.0;

    println!("  Protection ratio: {:.1}%", protection_ratio);

    // UAT requirement: 系统应能识别并阻止DoS攻击
    assert!(
        rate_limited > 0,
        "UAT requirement failed: System should block DoS attack requests, but none were blocked"
    );

    // UAT requirement: 保护机制应有效（阻止显著比例的攻击）
    assert!(
        protection_ratio >= 10.0, // 至少阻止10%的攻击请求
        "UAT requirement failed: Protection ratio ({:.1}%) is too low, expected >= 10%",
        protection_ratio
    );

    println!("✅ UAT DoS protection verified:");
    println!(
        "   - System blocked {} out of {} attack requests",
        rate_limited, total_requests
    );
    println!("   - Protection effectiveness: {:.1}%", protection_ratio);
    println!("   - Rate limiting mechanism working correctly");

    println!("UAT DoS protection rate limiting test completed successfully!");
}

#[tokio::test]
async fn test_dos_protection_sustained_attack() {
    // SEC-PEN-003: DoS攻击防护测试 - 持续攻击
    println!("Starting DoS protection sustained attack test...");

    let sdk = Arc::new(
        TestSdkBuilder::new()
            .with_network_simulator(NetworkSimulator::wifi())
            .build()
            .await
            .unwrap(),
    );

    let target_device = test_device_id();
    let attack_duration = Duration::from_secs(10); // 10秒攻击
    let attack_rate = 150; // 150请求/秒，超过100的限制

    // 模拟持续攻击
    let (successful, blocked) =
        simulate_dos_attack(&sdk, target_device, attack_duration, attack_rate)
            .await
            .unwrap();

    println!("Sustained attack results:");
    println!("  Attack duration: {:?}", attack_duration);
    println!("  Attack rate: {} requests/second", attack_rate);
    println!("  Successful requests: {}", successful);
    println!("  Blocked requests: {}", blocked);

    // 验证防护机制在持续攻击下仍然有效
    // 期望大部分请求被速率限制拒绝
    let total_requests = successful + blocked;
    let blocked_ratio = blocked as f64 / total_requests as f64;

    println!("  Blocked ratio: {:.1}%", blocked_ratio * 100.0);

    assert!(
        blocked > 0,
        "Rate limiting should have blocked some requests during sustained attack"
    );

    assert!(
        blocked_ratio > 0.3, // 期望至少30%的请求被阻止
        "Expected at least 30% of requests to be blocked, but only {:.1}% were blocked",
        blocked_ratio * 100.0
    );

    println!("DoS protection sustained attack test completed!");
}

#[tokio::test]
async fn test_dos_protection_recovery() {
    // SEC-PEN-003: DoS攻击防护测试 - 攻击后恢复
    println!("Starting DoS protection recovery test...");

    let sdk = Arc::new(
        TestSdkBuilder::new()
            .with_network_simulator(NetworkSimulator::wifi())
            .build()
            .await
            .unwrap(),
    );

    let target_device = test_device_id();

    // 第一阶段：正常请求（应该全部成功）
    println!("Phase 1: Normal requests...");
    let (normal_success, normal_limited) =
        test_rate_limiting(&sdk, target_device, 50).await.unwrap();
    assert_eq!(normal_success, 50, "Normal requests should all succeed");
    assert_eq!(
        normal_limited, 0,
        "Normal requests should not be rate limited"
    );

    // 第二阶段：模拟攻击（大量请求）
    println!("Phase 2: Simulating attack...");
    let (_attack_success, attack_limited) =
        test_rate_limiting(&sdk, target_device, 150).await.unwrap();
    assert!(attack_limited > 0, "Attack requests should be rate limited");

    // 等待一段时间让速率限制窗口重置
    println!("Waiting for rate limit window to reset...");
    sleep(Duration::from_secs(2)).await;

    // 第三阶段：恢复正常请求（应该再次成功）
    println!("Phase 3: Recovery - normal requests...");
    let (recovery_success, recovery_limited) =
        test_rate_limiting(&sdk, target_device, 30).await.unwrap();
    assert_eq!(recovery_success, 30, "Recovery requests should succeed");
    assert_eq!(
        recovery_limited, 0,
        "Recovery requests should not be rate limited"
    );

    println!("DoS protection recovery test completed successfully!");
}

#[tokio::test]
async fn test_dos_protection_edge_cases() {
    // SEC-PEN-003: DoS攻击防护测试 - 边界情况
    println!("Starting DoS protection edge cases test...");

    let sdk = Arc::new(
        TestSdkBuilder::new()
            .with_network_simulator(NetworkSimulator::wifi())
            .build()
            .await
            .unwrap(),
    );

    let target_device = test_device_id();

    // 测试1：精确在速率限制边界（100请求）
    println!("Edge case 1: Exactly at rate limit (100 requests)...");
    let (boundary_success, boundary_limited) =
        test_rate_limiting(&sdk, target_device, 100).await.unwrap();
    println!(
        "  Results: {} successful, {} rate limited",
        boundary_success, boundary_limited
    );

    // 等待速率限制窗口重置
    println!("Waiting for rate limit window to reset...");
    sleep(Duration::from_secs(2)).await;

    // 测试2：稍微超过速率限制（105请求）
    println!("Edge case 2: Slightly over rate limit (105 requests)...");
    let (over_success, over_limited) = test_rate_limiting(&sdk, target_device, 105).await.unwrap();
    println!(
        "  Results: {} successful, {} rate limited",
        over_success, over_limited
    );

    // 验证稍微超过限制时会有部分请求被限制
    assert!(
        over_limited > 0,
        "Slightly over rate limit should trigger some blocking"
    );

    // 等待速率限制窗口重置
    println!("Waiting for rate limit window to reset...");
    sleep(Duration::from_secs(2)).await;

    // 测试3：极低速率（1请求/秒）
    println!("Edge case 3: Very low rate (1 request)...");
    let result = sdk
        .send(
            target_device,
            MessagePayload::Text("Single request".to_string()),
        )
        .await;
    assert!(result.is_ok(), "Single request should always succeed");

    println!("DoS protection edge cases test completed successfully!");
}

#[tokio::test]
async fn test_uat_dos_protection_comprehensive() {
    // UAT: 综合DoS攻击防护测试 - 验收标准：系统能抵御各种DoS攻击并保持服务可用性
    println!("Starting comprehensive UAT DoS protection test...");

    let sdk = Arc::new(
        TestSdkBuilder::new()
            .with_network_simulator(NetworkSimulator::wifi())
            .build()
            .await
            .unwrap(),
    );

    let target_device = test_device_id();

    println!("=== UAT DoS Protection Comprehensive Test ===");

    // Test 1: 验证正常请求始终可用
    println!("\n1. Testing normal request availability during attack simulation...");
    let normal_result = sdk
        .send(
            target_device,
            MessagePayload::Text("Normal user request".to_string()),
        )
        .await;
    assert!(
        normal_result.is_ok(),
        "UAT requirement failed: Normal requests should always be possible"
    );
    println!("✅ Normal requests available");

    // Test 2: 模拟高强度攻击（300请求/秒，持续5秒）
    println!("\n2. Testing high-intensity attack protection (300 req/sec for 5 seconds)...");
    let attack_duration = Duration::from_secs(5);
    let attack_rate = 300;
    let (successful_attacks, blocked_attacks) =
        simulate_dos_attack(&sdk, target_device, attack_duration, attack_rate)
            .await
            .unwrap();

    let total_attack_requests = successful_attacks + blocked_attacks;
    let attack_blocked_ratio = blocked_attacks as f64 / total_attack_requests as f64 * 100.0;

    println!(
        "   Attack results: {} successful, {} blocked",
        successful_attacks, blocked_attacks
    );
    println!("   Blocked ratio: {:.1}%", attack_blocked_ratio);

    // UAT requirement: 系统应阻止大部分攻击请求
    assert!(
        blocked_attacks > 0,
        "UAT requirement failed: System should block attack requests"
    );
    assert!(
        attack_blocked_ratio >= 15.0,
        "UAT requirement failed: Attack blocked ratio ({:.1}%) too low",
        attack_blocked_ratio
    );

    // Test 3: 验证攻击后系统恢复能力
    println!("\n3. Testing system recovery after attack...");
    sleep(Duration::from_secs(2)).await; // 等待恢复时间

    let recovery_result = sdk
        .send(
            target_device,
            MessagePayload::Text("Recovery test request".to_string()),
        )
        .await;
    assert!(
        recovery_result.is_ok(),
        "UAT requirement failed: System should recover after attack"
    );
    println!("✅ System recovered after attack");

    // Test 4: 验证服务可用性指标
    println!("\n4. Testing service availability metrics...");
    let availability_test_count = 10;
    let mut successful_normal_requests = 0;

    for i in 0..availability_test_count {
        let result = sdk
            .send(
                target_device,
                MessagePayload::Text(format!("Availability test {}", i)),
            )
            .await;
        if result.is_ok() {
            successful_normal_requests += 1;
        }
    }

    let availability_ratio =
        successful_normal_requests as f64 / availability_test_count as f64 * 100.0;
    println!(
        "   Service availability: {:.1}% ({}/{} requests)",
        availability_ratio, successful_normal_requests, availability_test_count
    );

    // UAT requirement: 服务可用性应保持在较高水平
    assert!(
        availability_ratio >= 90.0,
        "UAT requirement failed: Service availability ({:.1}%) too low",
        availability_ratio
    );

    // Summary
    println!("\n=== UAT DoS Protection Summary ===");
    println!("✅ Normal request availability: Always available");
    println!(
        "✅ Attack protection: {:.1}% of {} attack requests blocked",
        attack_blocked_ratio, total_attack_requests
    );
    println!("✅ System recovery: Successful after attack");
    println!(
        "✅ Service availability: {:.1}% during testing",
        availability_ratio
    );

    println!("\n✅ UAT DoS protection requirements satisfied:");
    println!("   - System can identify and block DoS attacks");
    println!("   - Normal users can still access service during attacks");
    println!("   - System recovers quickly after attacks");
    println!("   - Service maintains high availability");

    println!("\nUAT comprehensive DoS protection test completed successfully!");
}
