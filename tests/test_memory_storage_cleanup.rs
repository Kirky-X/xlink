#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};
    use std::sync::Arc;
    use uuid::Uuid;
    use xpush::core::traits::Storage;
    use xpush::core::types::{DeviceId, Message, MessagePayload, MessagePriority};
    use xpush::storage::memory_store::MemoryStorage;

    async fn get_memory_usage() -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .stdout(Stdio::piped())
            .output()?;

        let memory_kb = String::from_utf8(output.stdout)?.trim().parse::<u64>()?;

        Ok(memory_kb)
    }

    #[tokio::test]
    async fn test_memory_storage_cleanup() {
        let initial_memory = get_memory_usage().await.unwrap();
        println!("Initial memory: {} KB", initial_memory);

        // Create and fill MemoryStorage similar to SDK usage
        {
            let storage = Arc::new(MemoryStorage::new());
            let device_id = DeviceId::new();

            // Fill with data similar to SDK usage
            for i in 0..1000 {
                let message = Message {
                    id: Uuid::new_v4(),
                    sender: device_id,
                    recipient: device_id,
                    group_id: None,
                    payload: MessagePayload::Text(format!("Message {}", i)),
                    priority: MessagePriority::Normal,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    require_ack: true,
                };
                storage.save_message(&message).await.unwrap();
                storage.save_pending_message(&message).await.unwrap();
                storage
                    .save_audit_log(format!("Audit log {}", i))
                    .await
                    .unwrap();
            }

            let after_fill = get_memory_usage().await.unwrap();
            println!(
                "After filling storage: {} KB (increase: {} KB)",
                after_fill,
                after_fill - initial_memory
            );

            // Test cleanup_storage method
            let removed = storage.cleanup_storage(1024).await.unwrap();
            println!("Cleanup removed {} bytes", removed);

            let after_cleanup = get_memory_usage().await.unwrap();
            let cleanup_change = after_cleanup.saturating_sub(after_fill);
            println!(
                "After cleanup: {} KB (change: {} KB)",
                after_cleanup, cleanup_change
            );

            // Drop the storage (should trigger Drop implementation)
            drop(storage);
        }

        // Force cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        {
            let _force_gc: Vec<u8> = vec![0; 1024 * 1024];
        }

        let final_memory = get_memory_usage().await.unwrap();
        let net_change = final_memory.saturating_sub(initial_memory);
        println!(
            "Final memory: {} KB (net change: {} KB)",
            final_memory, net_change
        );

        if net_change > 1024 {
            println!("✗ FAIL: MemoryStorage cleanup issue detected");
        } else {
            println!("✓ PASS: MemoryStorage cleaned up properly");
        }
    }
}
