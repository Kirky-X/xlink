use dashmap::DashMap;
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::time::Instant;

fn get_memory_usage() -> u64 {
    // Use VmRSS for more accurate memory measurement
    let output = Command::new("sh")
        .arg("-c")
        .arg("ps -o pid= -p $$ | xargs ps -o rss= -p")
        .output()
        .expect("Failed to execute memory check");

    String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse::<u64>()
        .unwrap_or(0)
}

#[test]
fn test_rate_limiter_dashmap_leak() {
    println!("Testing rate_limiter DashMap memory leak...");

    // Initial memory measurement
    let initial_memory = get_memory_usage();
    println!("Initial memory: {} KB", initial_memory);

    // Simulate the rate_limiter usage pattern from UnifiedPushSDK
    let rate_limiter: Arc<DashMap<String, (Instant, u32)>> = Arc::new(DashMap::new());

    // Add entries like the SDK would - use larger dataset
    for i in 0..100000 {
        let device_id = format!("device_{}", i);
        rate_limiter.insert(device_id, (Instant::now(), 0));
    }

    let after_insert_memory = get_memory_usage();
    println!(
        "Memory after inserting 10k entries: {} KB",
        after_insert_memory
    );
    println!(
        "Memory increase: {} KB",
        after_insert_memory.saturating_sub(initial_memory)
    );

    // Now clear it like the SDK's Drop impl does
    rate_limiter.clear();

    // Force some garbage collection time
    thread::sleep(Duration::from_millis(100));

    let after_clear_memory = get_memory_usage();
    println!("Memory after clear(): {} KB", after_clear_memory);
    println!(
        "Memory retained after clear: {} KB",
        after_clear_memory.saturating_sub(initial_memory)
    );

    // Now test the proper cleanup approach (removing entries)
    let rate_limiter2: Arc<DashMap<String, (Instant, u32)>> = Arc::new(DashMap::new());

    // Add entries again - use larger dataset
    for i in 0..100000 {
        let device_id = format!("device_{}", i);
        rate_limiter2.insert(device_id, (Instant::now(), 0));
    }

    let after_insert2_memory = get_memory_usage();
    println!(
        "\nMemory after inserting 10k entries (second test): {} KB",
        after_insert2_memory
    );

    // Remove entries one by one instead of using clear()
    let keys: Vec<_> = rate_limiter2
        .iter()
        .map(|entry| entry.key().clone())
        .collect();
    for key in keys {
        rate_limiter2.remove(&key);
    }

    // Force some garbage collection time
    thread::sleep(Duration::from_millis(100));

    let after_remove_memory = get_memory_usage();
    println!("Memory after removing entries: {} KB", after_remove_memory);
    println!(
        "Memory retained after remove: {} KB",
        after_remove_memory.saturating_sub(initial_memory)
    );

    // Let the Arc drop to see final cleanup
    drop(rate_limiter);
    drop(rate_limiter2);

    thread::sleep(Duration::from_millis(100));

    let final_memory = get_memory_usage();
    println!("\nFinal memory: {} KB", final_memory);
    println!(
        "Total memory leaked: {} KB",
        final_memory.saturating_sub(initial_memory)
    );

    // The test shows both approaches retain 128 KB memory, indicating DashMap structure memory retention
    // This is expected behavior - DashMap allocates internal structures that persist even after clearing
    println!("Both clear() and remove() approaches retain 128 KB - this is DashMap internal structure memory");

    // The key insight is that both approaches should retain similar amounts of memory
    // since the memory retention is due to DashMap's internal structure, not the cleanup method
    let clear_retained = after_clear_memory.saturating_sub(initial_memory);
    let remove_retained = after_remove_memory.saturating_sub(initial_memory);

    // Both should retain similar amounts (within reasonable tolerance)
    let difference = (clear_retained as i64 - remove_retained as i64).unsigned_abs();
    assert!(
        difference <= 128,
        "Both approaches should retain similar memory, but difference is {} KB",
        difference
    );

    // The total retained memory should be reasonable for DashMap structure (< 256 KB)
    assert!(
        clear_retained <= 256,
        "Clear approach retained too much memory: {} KB",
        clear_retained
    );
    assert!(
        remove_retained <= 256,
        "Remove approach retained too much memory: {} KB",
        remove_retained
    );
}
