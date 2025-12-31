use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use xlink::channels::memory::MemoryChannel;
use xlink::core::types::{ChannelType, DeviceCapabilities, DeviceId, DeviceType};
use xlink::XLink;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // 1. Setup SDK
    let device_id = DeviceId::new();
    let caps = DeviceCapabilities {
        device_id,
        device_type: DeviceType::Smartphone,
        device_name: "Discovery Demo".to_string(),
        supported_channels: HashSet::from([ChannelType::BluetoothLE]),
        battery_level: Some(85),
        is_charging: false,
        data_cost_sensitive: false,
    };

    struct DemoHandler;
    #[async_trait::async_trait]
    impl xlink::core::traits::MessageHandler for DemoHandler {
        async fn handle_message(
            &self,
            _msg: xlink::core::types::Message,
        ) -> xlink::core::error::Result<()> {
            Ok(())
        }
    }

    let mem_channel = Arc::new(MemoryChannel::new(Arc::new(DemoHandler), 0));
    let sdk = XLink::new(caps, vec![mem_channel.clone()]).await?;
    sdk.start().await?;

    log::info!("SDK started. Waiting for background discovery events...");

    // 2. Simulate Background Discovery
    // This would normally be triggered by OS-level BLE/WiFi scan results
    let discovered_device_id = DeviceId::new();
    log::info!(
        "Simulating background discovery of device: {}...",
        discovered_device_id
    );

    // Call the background discovery simulation interface
    sdk.simulate_background_discovery(discovered_device_id)
        .await?;

    // 3. Verify Discovery
    sleep(Duration::from_millis(100)).await;
    if let Some(caps) = sdk
        .capability_manager()
        .get_remote_device(discovered_device_id)
    {
        log::info!(
            "Discovered device: {}, type: {:?}",
            caps.device_name,
            caps.device_type
        );
    } else {
        log::error!("Failed to discover device in background simulation.");
    }

    log::info!("Background discovery demo completed.");
    Ok(())
}
