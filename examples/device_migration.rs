use std::collections::HashSet;
use std::sync::Arc;
use xpush::channels::memory::MemoryChannel;
use xpush::core::types::{
    DeviceCapabilities, DeviceId, DeviceType, ChannelType
};
use xpush::UnifiedPushSDK;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // 1. Setup SDK on Old Device
    let old_id = DeviceId::new();
    let caps = DeviceCapabilities {
        device_id: old_id,
        device_type: DeviceType::Smartphone,
        device_name: "Old Phone".to_string(),
        supported_channels: HashSet::from([ChannelType::Lan]),
        battery_level: Some(100),
        is_charging: true,
        data_cost_sensitive: false,
    };

    struct DemoHandler;
    #[async_trait::async_trait]
    impl xpush::core::traits::MessageHandler for DemoHandler {
        async fn handle_message(&self, _msg: xpush::core::types::Message) -> xpush::core::error::Result<()> {
            Ok(())
        }
    }
    
    let mem_channel = Arc::new(MemoryChannel::new(Arc::new(DemoHandler), 0));
    let sdk_old = UnifiedPushSDK::new(caps.clone(), vec![mem_channel.clone()]).await?;
    sdk_old.start().await?;

    log::info!("Old device initialized. ID: {}", old_id);

    // 2. Export state from old device
    log::info!("Exporting SDK state from old device...");
    let state_data = sdk_old.export_sdk_state()?;
    log::info!("State exported ({} bytes).", state_data.len());

    // 3. Initialize new device (simulated)
    let new_id = DeviceId::new();
    let mut new_caps = caps.clone();
    new_caps.device_id = new_id;
    new_caps.device_name = "New Phone".to_string();

    let mut sdk_new = UnifiedPushSDK::new(new_caps, vec![mem_channel.clone()]).await?;
    sdk_new.start().await?;
    log::info!("New device initialized. ID: {}", new_id);

    // 4. Import state to new device
    log::info!("Importing SDK state to new device...");
    sdk_new.import_sdk_state(&state_data)?;
    log::info!("State imported successfully. Keys and identity migrated.");

    log::info!("Device migration demo completed.");
    Ok(())
}
