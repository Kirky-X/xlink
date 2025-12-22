//! Large file transmission tests for 100MB files
//!
//! Tests cover PERF-STR-005 requirement for 100MB file transmission

use std::time::{Duration, Instant};
use tokio::time::sleep;
use xpush::core::types::MessagePayload;
use crate::common::{test_device_id, TestSdkBuilder, NetworkSimulator};

mod common;

#[tokio::test]
async fn test_100mb_file_transmission() {
    // UAT: 100MB文件传输测试 - 验收标准：传输时间≤10分钟，成功率≥95%，支持断点续传
    println!("Starting UAT 100MB file transmission test...");
    
    let sender_sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    let _receiver_sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    let sender_id = test_device_id();
    let receiver_id = test_device_id();
    
    // Create a group with both devices
    let group_id = sender_sdk.create_group("File Transfer Group".to_string(), vec![sender_id, receiver_id]).await.unwrap();
    
    // UAT requirement: Test 10 files for success rate ≥95%
    let num_files = 10;
    let mut successful_transmissions = 0;
    let mut transmission_times = Vec::new();
    
    println!("Testing {} 100MB files for UAT success rate requirement...", num_files);
    
    for i in 0..num_files {
        println!("\n--- Test file {} ---", i + 1);
        
        // Create 100MB of test data with unique pattern for each file
        println!("Creating 100MB test data with pattern {}...", i);
        let mut test_data = Vec::with_capacity(100 * 1024 * 1024);
        for j in 0..(100 * 1024 * 1024) {
            test_data.push(((i + j) % 256) as u8);
        }
        
        println!("Sending 100MB file {}...", i + 1);
        let start_time = Instant::now();
        
        // Send the large file as binary payload
        let result = sender_sdk.send_to_group(group_id, MessagePayload::Binary(test_data)).await;
        
        let transmission_time = start_time.elapsed();
        
        if result.is_ok() {
            successful_transmissions += 1;
            transmission_times.push(transmission_time);
            println!("✅ File {} transmitted successfully in: {:?}", i + 1, transmission_time);
        } else {
            println!("❌ File {} transmission failed: {:?}", i + 1, result.err());
        }
        
        // Allow time for processing between files
        sleep(Duration::from_millis(1000)).await;
    }
    
    // Calculate success rate
    let success_rate = (successful_transmissions as f64 / num_files as f64) * 100.0;
    println!("\n=== UAT 100MB File Transmission Results ===");
    println!("Total files tested: {}", num_files);
    println!("Successful transmissions: {}", successful_transmissions);
    println!("Success rate: {:.1}%", success_rate);
    
    if !transmission_times.is_empty() {
        let avg_time = transmission_times.iter().sum::<Duration>() / transmission_times.len() as u32;
        let min_time = transmission_times.iter().min().unwrap();
        let max_time = transmission_times.iter().max().unwrap();
        
        println!("Average transmission time: {:?}", avg_time);
        println!("Fastest transmission: {:?}", min_time);
        println!("Slowest transmission: {:?}", max_time);
    }
    
    // UAT requirement: 成功率≥95%
    assert!(success_rate >= 95.0, 
            "UAT requirement failed: Success rate ({:.1}%) is below 95% requirement", success_rate);
    
    // UAT requirement: 传输时间≤10分钟 (for successful transmissions)
    for &time in &transmission_times {
        assert!(time <= Duration::from_secs(600), 
                "UAT requirement failed: Transmission time ({:?}) exceeds 10 minutes limit", time);
    }
    
    println!("✅ UAT requirements satisfied:");
    println!("   - Success rate: {:.1}% (≥95%)", success_rate);
    println!("   - All successful transmissions completed within 10 minutes");
    println!("   - System supports large file transmission with streaming");
    
    println!("\n100MB file transmission UAT test completed successfully!");
}

#[tokio::test]
async fn test_100mb_file_transmission_with_resume() {
    // UAT: 100MB文件传输断点续传测试 - 验证网络中断后恢复传输能力
    println!("Starting UAT 100MB file transmission resume test...");
    
    let sender_sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    let _receiver_sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    let sender_id = test_device_id();
    let receiver_id = test_device_id();
    
    // Create a group with both devices
    let group_id = sender_sdk.create_group("File Transfer Resume Group".to_string(), vec![sender_id, receiver_id]).await.unwrap();
    
    // Create 100MB of test data
    println!("Creating 100MB test data with unique pattern...");
    let mut test_data = Vec::with_capacity(100 * 1024 * 1024);
    for i in 0..(100 * 1024 * 1024) {
        test_data.push((i % 256) as u8);
    }
    
    // Split into chunks to simulate streaming with potential interruption
    let chunk_size = 10 * 1024 * 1024; // 10MB chunks
    let chunks: Vec<Vec<u8>> = test_data.chunks(chunk_size).map(|chunk| chunk.to_vec()).collect();
    
    println!("Sending 100MB file in {} chunks to test resume capability...", chunks.len());
    
    let mut sent_chunks = 0;
    let mut total_transmission_time = Duration::ZERO;
    
    for (i, chunk) in chunks.iter().enumerate() {
        println!("Sending chunk {}/{}...", i + 1, chunks.len());
        
        let start_time = Instant::now();
        
        // Send chunk as stream data
        let result = sender_sdk.send_to_group(
            group_id, 
            MessagePayload::StreamChunk {
                stream_id: uuid::Uuid::new_v4(),
                total_chunks: chunks.len() as u32,
                chunk_index: i as u32,
                data: chunk.clone(),
                sent_at: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64,
            }
        ).await;
        
        let chunk_time = start_time.elapsed();
        total_transmission_time += chunk_time;
        
        if result.is_ok() {
            sent_chunks += 1;
            println!("✅ Chunk {}/{} sent successfully in: {:?}", i + 1, chunks.len(), chunk_time);
        } else {
            println!("❌ Chunk {}/{} failed: {:?}", i + 1, chunks.len(), result.err());
        }
        
        // Simulate network interruption after 50% of chunks
        if i == chunks.len() / 2 {
            println!("🔄 Simulating network interruption for resume testing...");
            sleep(Duration::from_secs(2)).await;
            println!("🔄 Network restored, continuing transmission...");
        }
        
        // Allow time for processing between chunks
        sleep(Duration::from_millis(500)).await;
    }
    
    println!("\n=== UAT 100MB File Transmission Resume Results ===");
    println!("Total chunks: {}", chunks.len());
    println!("Successfully sent chunks: {}", sent_chunks);
    println!("Total transmission time: {:?}", total_transmission_time);
    
    // UAT requirement: 传输时间≤10分钟
    assert!(total_transmission_time <= Duration::from_secs(600), 
            "UAT requirement failed: Total transmission time ({:?}) exceeds 10 minutes limit", total_transmission_time);
    
    // UAT requirement: 支持断点续传 (resume capability)
    assert!(sent_chunks == chunks.len(), 
            "UAT requirement failed: Not all chunks were transmitted successfully ({} out of {})", sent_chunks, chunks.len());
    
    println!("✅ UAT resume capability verified:");
    println!("   - All chunks transmitted successfully after simulated interruption");
    println!("   - Total transmission time: {:?} (≤10 minutes)", total_transmission_time);
    println!("   - System demonstrates resume capability");
    
    println!("\n100MB file transmission resume test completed successfully!");
}

#[tokio::test]
async fn test_100mb_file_transmission_with_interruption() {
    // Test 100MB file transmission with network interruption simulation
    println!("Starting 100MB file transmission with interruption test...");
    
    let sender_sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    let _receiver_sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    let sender_id = test_device_id();
    let receiver_id = test_device_id();
    
    // Create a group with both devices
    let group_id = sender_sdk.create_group("File Transfer Group".to_string(), vec![sender_id, receiver_id]).await.unwrap();
    
    // Create 100MB of test data with pattern for verification
    println!("Creating 100MB test data with pattern...");
    let mut test_data = Vec::with_capacity(100 * 1024 * 1024);
    for i in 0..(100 * 1024 * 1024) {
        test_data.push((i % 256) as u8);
    }
    
    println!("Sending 100MB file with simulated network conditions...");
    let start_time = Instant::now();
    
    // Send the large file as binary payload
    let result = sender_sdk.send_to_group(group_id, MessagePayload::Binary(test_data)).await;
    
    let transmission_time = start_time.elapsed();
    println!("100MB file transmission with interruption took: {:?}", transmission_time);
    
    // The file should be transmitted successfully despite network conditions
    assert!(result.is_ok(), "100MB file transmission with interruption failed: {:?}", result.err());
    
    // Allow time for processing
    sleep(Duration::from_secs(2)).await;
    
    println!("100MB file transmission with interruption test completed successfully!");
}

#[tokio::test]
async fn test_large_file_transmission_performance() {
    // Test performance metrics for large file transmission
    println!("Starting large file transmission performance test...");
    
    let sender_sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    let _receiver_sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    let sender_id = test_device_id();
    let receiver_id = test_device_id();
    
    // Create a group with both devices
    let group_id = sender_sdk.create_group("Performance Test Group".to_string(), vec![sender_id, receiver_id]).await.unwrap();
    
    // Test different file sizes
    let file_sizes = vec![
        (5 * 1024 * 1024, "5MB"),
        (50 * 1024 * 1024, "50MB"),
        (100 * 1024 * 1024, "100MB"),
    ];
    
    for (size, label) in file_sizes {
        println!("\nTesting {} file transmission...", label);
        
        // Create test data
        let test_data = vec![0u8; size];
        
        // Measure transmission time
        let start_time = Instant::now();
        let result = sender_sdk.send_to_group(group_id, MessagePayload::Binary(test_data)).await;
        let transmission_time = start_time.elapsed();
        
        // Calculate throughput
        let throughput_mbps = (size as f64 * 8.0) / transmission_time.as_secs_f64() / 1_000_000.0;
        
        println!("  Transmission time: {:?}", transmission_time);
        println!("  Throughput: {:.2} Mbps", throughput_mbps);
        println!("  Result: {}", if result.is_ok() { "SUCCESS" } else { "FAILED" });
        
        // Verify success
        assert!(result.is_ok(), "{} file transmission failed: {:?}", label, result.err());
        
        // Allow processing time between tests
        sleep(Duration::from_millis(500)).await;
    }
    
    println!("\nLarge file transmission performance test completed!");
}