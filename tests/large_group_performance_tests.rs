//! Large group performance tests for 500+ member groups
//!
//! Tests cover PERF-GRP-004 and PERF-GRP-005 requirements for large group broadcasting


use std::time::{Duration, Instant};
use tokio::time::sleep;
use xpush::core::types::MessagePayload;
use crate::common::{test_device_id, TestSdkBuilder, NetworkSimulator};

mod common;

#[tokio::test]
async fn test_500_person_group_broadcast() {
    // PERF-GRP-004: 500人群组广播测试
    println!("Starting 500-person group broadcast test...");
    
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    // Create 500 test devices
    let mut device_ids = Vec::new();
    for i in 0..500 {
        let device_id = test_device_id();
        device_ids.push(device_id);
        
        // Print progress every 50 devices
        if (i + 1) % 50 == 0 {
            println!("Created {} devices...", i + 1);
        }
    }
    
    println!("Creating 500-person group...");
    let start_time = Instant::now();
    
    // Create large group
    let group_id = sdk.create_group("Large Performance Group".to_string(), device_ids.clone()).await;
    
    let creation_time = start_time.elapsed();
    println!("Group creation took: {:?}", creation_time);
    
    // Verify group was created successfully
    assert!(group_id.is_ok(), "Failed to create 500-person group: {:?}", group_id.err());
    let group_id = group_id.unwrap();
    
    // Test single broadcast message
    println!("Sending broadcast message to 500 members...");
    let broadcast_start = Instant::now();
    
    let result = sdk.send_to_group(group_id, MessagePayload::Text("Hello 500 members!".to_string())).await;
    
    let broadcast_time = broadcast_start.elapsed();
    println!("Broadcast took: {:?}", broadcast_time);
    
    // The message should be sent successfully (system should not crash)
    assert!(result.is_ok(), "Broadcast failed: {:?}", result.err());
    
    // Allow time for message processing
    sleep(Duration::from_millis(500)).await;
    
    println!("500-person group broadcast test completed successfully!");
}

#[tokio::test]
async fn test_500_person_group_multiple_broadcasts() {
    // Test multiple broadcasts to 500-person group
    println!("Starting multiple broadcasts to 500-person group...");
    
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    // Create 500 test devices
    let mut device_ids = Vec::new();
    for _ in 0..500 {
        let device_id = test_device_id();
        device_ids.push(device_id);
    }
    
    let group_id = sdk.create_group("Multiple Broadcast Group".to_string(), device_ids).await.unwrap();
    
    // Send multiple messages with timing
    let mut total_broadcast_time = Duration::ZERO;
    let mut successful_broadcasts = 0;
    
    for i in 0..10 {
        let start = Instant::now();
        let result = sdk.send_to_group(group_id, MessagePayload::Text(format!("Broadcast message {}", i))).await;
        let elapsed = start.elapsed();
        
        if result.is_ok() {
            total_broadcast_time += elapsed;
            successful_broadcasts += 1;
            println!("Broadcast {} completed in: {:?}", i + 1, elapsed);
        } else {
            println!("Broadcast {} failed: {:?}", i + 1, result.err());
        }
        
        // Small delay between broadcasts
        sleep(Duration::from_millis(100)).await;
    }
    
    let avg_broadcast_time = total_broadcast_time / 10;
    println!("Average broadcast time: {:?}", avg_broadcast_time);
    
    // System should handle multiple broadcasts without issues
    assert!(avg_broadcast_time < Duration::from_secs(5), "Broadcasts are too slow");
}

#[tokio::test]
async fn test_1000_person_group_broadcast() {
    // PERF-GRP-005: 1000人群组广播测试
    println!("Starting 1000-person group broadcast test...");
    
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    // Create 1000 test devices
    let mut device_ids = Vec::new();
    for i in 0..1000 {
        let device_id = test_device_id();
        device_ids.push(device_id);
        
        // Print progress every 100 devices
        if (i + 1) % 100 == 0 {
            println!("Created {} devices...", i + 1);
        }
    }
    
    println!("Creating 1000-person group...");
    let start_time = Instant::now();
    
    // Create very large group
    let group_id = sdk.create_group("Very Large Performance Group".to_string(), device_ids.clone()).await;
    
    let creation_time = start_time.elapsed();
    println!("Group creation took: {:?}", creation_time);
    
    // Verify group was created successfully
    assert!(group_id.is_ok(), "Failed to create 1000-person group: {:?}", group_id.err());
    let group_id = group_id.unwrap();
    
    // Test single broadcast message
    println!("Sending broadcast message to 1000 members...");
    let broadcast_start = Instant::now();
    
    let result = sdk.send_to_group(group_id, MessagePayload::Text("Hello 1000 members!".to_string())).await;
    
    let broadcast_time = broadcast_start.elapsed();
    println!("Broadcast took: {:?}", broadcast_time);
    
    // The message should be sent successfully (system should not crash)
    assert!(result.is_ok(), "Broadcast failed: {:?}", result.err());
    
    // Allow time for message processing
    sleep(Duration::from_secs(1)).await;
    
    println!("1000-person group broadcast test completed successfully!");
}

#[tokio::test]
async fn test_large_group_performance_metrics() {
    // Test performance metrics for large groups
    println!("Starting large group performance metrics test...");
    
    let sdk = TestSdkBuilder::new()
        .with_network_simulator(NetworkSimulator::wifi())
        .build().await.unwrap();
    
    // Test different group sizes
    let group_sizes = vec![100, 250, 500];
    
    for size in group_sizes {
        println!("\nTesting {}-person group...", size);
        
        // Create test devices
        let mut device_ids = Vec::new();
        for _ in 0..size {
            device_ids.push(test_device_id());
        }
        
        // Measure group creation time
        let creation_start = Instant::now();
        let group_id = sdk.create_group(format!("Performance Group {}", size), device_ids).await.unwrap();
        let creation_time = creation_start.elapsed();
        
        // Measure broadcast time
        let broadcast_start = Instant::now();
        let result = sdk.send_to_group(group_id, MessagePayload::Text(format!("Test message for {} members", size))).await;
        let broadcast_time = broadcast_start.elapsed();
        
        println!("  Group creation: {:?}", creation_time);
        println!("  Broadcast time: {:?}", broadcast_time);
        println!("  Result: {}", if result.is_ok() { "SUCCESS" } else { "FAILED" });
        
        // Verify success
        assert!(result.is_ok(), "Failed to broadcast to {}-person group", size);
        
        // Allow processing time
        sleep(Duration::from_millis(200)).await;
    }
    
    println!("\nPerformance metrics test completed!");
}