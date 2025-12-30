use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use xlink::channels::memory::MemoryChannel;
use xlink::core::types::{
    ChannelState, ChannelType, DeviceCapabilities, DeviceId, DeviceType, MessagePayload,
};
use xlink::UnifiedPushSDK;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // 1. Setup SDK
    let device_id = DeviceId::new();
    let caps = DeviceCapabilities {
        device_id,
        device_type: DeviceType::Smartphone,
        device_name: "Test Phone".to_string(),
        supported_channels: HashSet::from([ChannelType::BluetoothLE, ChannelType::Lan]),
        battery_level: Some(80),
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

    let mem_channel = Arc::new(MemoryChannel::new(Arc::new(DemoHandler), 50));
    let sdk = UnifiedPushSDK::new(caps, vec![mem_channel.clone()]).await?;
    sdk.start().await?;

    // 2. Setup a target device
    let target_id = DeviceId::new();
    sdk.capability_manager()
        .register_remote_device(DeviceCapabilities {
            device_id: target_id,
            device_type: DeviceType::Smartphone,
            device_name: "Target Phone".to_string(),
            supported_channels: HashSet::from([ChannelType::BluetoothLE, ChannelType::Lan]),
            battery_level: Some(100),
            is_charging: true,
            data_cost_sensitive: false,
        });

    // 3. Scenario: Bluetooth is fast but WiFi is available
    log::info!("--- Scenario 1: Both channels available, selecting best ---");
    sdk.capability_manager().update_channel_state(
        target_id,
        ChannelType::BluetoothLE,
        ChannelState {
            available: true,
            rtt_ms: 50,
            ..Default::default()
        },
    );
    sdk.capability_manager().update_channel_state(
        target_id,
        ChannelType::Lan,
        ChannelState {
            available: true,
            rtt_ms: 10,
            ..Default::default()
        },
    );

    log::info!("Sending message via best channel...");
    sdk.send(target_id, MessagePayload::Text("Message 1".to_string()))
        .await?;

    // 5. 查看流量统计
    let stats = sdk.router().get_traffic_stats();
    log::info!("Traffic statistics: {:?}", stats);

    // 4. Scenario: LAN fails, switching to Bluetooth
    log::info!("--- Scenario 2: LAN fails, automatic switching to Bluetooth ---");
    sdk.capability_manager().update_channel_state(
        target_id,
        ChannelType::Lan,
        ChannelState {
            available: false,
            ..Default::default()
        },
    );

    log::info!("Sending message via fallback channel...");
    sdk.send(target_id, MessagePayload::Text("Message 2".to_string()))
        .await?;

    // 5. Scenario: Data cost sensitivity
    log::info!("--- Scenario 3: Cost sensitive mode ---");
    // Enable cost sensitivity in router or capabilities (if supported)
    // For demo, we just simulate the behavior.
    log::info!("Simulation: Router will avoid metered channels when cost sensitivity is high.");

    sleep(Duration::from_millis(200)).await;
    log::info!("Channel switching demo completed.");
    Ok(())
}
