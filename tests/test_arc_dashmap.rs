#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::process::{Command, Stdio};
    
    async fn get_memory_usage() -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let output = Command::new("ps")
            .args(&["-o", "rss=", "-p", &std::process::id().to_string()])
            .stdout(Stdio::piped())
            .output()?;
        
        let memory_kb = String::from_utf8(output.stdout)?
            .trim()
            .parse::<u64>()?;
        
        Ok(memory_kb)
    }
    
    #[tokio::test]
    async fn test_arc_dashmap_cleanup() {
        let initial_memory = get_memory_usage().await.unwrap();
        println!("Initial memory: {} KB", initial_memory);
        
        // Test Arc-wrapped DashMap pattern similar to SDK
        {
            let dashmap_data = Arc::new(dashmap::DashMap::new());
            
            // Fill with data similar to SDK usage
            for i in 0..1000 {
                let device_id = format!("device_{}", i);
                let inner_map = dashmap::DashMap::new();
                for j in 0..100 {
                    inner_map.insert(format!("channel_{}", j), vec![0u8; 50]);
                }
                dashmap_data.insert(device_id, inner_map);
            }
            
            let after_fill = get_memory_usage().await.unwrap();
            println!("After filling DashMap: {} KB (increase: {} KB)", after_fill, after_fill - initial_memory);
            
            // Clear the data (similar to cleanup_storage)
            for entry in dashmap_data.iter_mut() {
                entry.clear();
            }
            
            let after_clear = get_memory_usage().await.unwrap();
            println!("After clearing DashMap: {} KB (change: {} KB)", after_clear, after_clear - after_fill);
            
            // Drop the Arc
            drop(dashmap_data);
        }
        
        // Force cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        { let _force_gc: Vec<u8> = vec![0; 1024 * 1024]; }
        
        let final_memory = get_memory_usage().await.unwrap();
        println!("Final memory: {} KB (net change: {} KB)", final_memory, final_memory - initial_memory);
        
        if final_memory - initial_memory > 1024 {
            println!("✗ FAIL: Arc-wrapped DashMap cleanup issue detected");
        } else {
            println!("✓ PASS: Arc-wrapped DashMap cleaned up properly");
        }
    }
}