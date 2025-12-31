use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use xlink::channels::memory::MemoryChannel;
use xlink::core::types::{ChannelType, DeviceCapabilities, DeviceId, DeviceType, MessagePayload};
use xlink::XLink;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // 1. Setup Alice's SDK
    let alice_id = DeviceId::new();
    let alice_caps = DeviceCapabilities {
        device_id: alice_id,
        device_type: DeviceType::Smartphone,
        device_name: "Alice Phone".to_string(),
        supported_channels: HashSet::from([ChannelType::Lan]),
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
    let sdk = XLink::new(alice_caps, vec![mem_channel.clone()]).await?;
    sdk.start().await?;

    log::info!("Alice SDK started. ID: {}", alice_id);

    // 2. Setup Bob and Carol
    let bob_id = DeviceId::new();
    let carol_id = DeviceId::new();

    // Register their capabilities
    sdk.capability_manager()
        .register_remote_device(DeviceCapabilities {
            device_id: bob_id,
            device_type: DeviceType::Smartphone,
            device_name: "Bob Phone".to_string(),
            supported_channels: HashSet::from([ChannelType::Lan]),
            battery_level: Some(90),
            is_charging: false,
            data_cost_sensitive: false,
        });

    sdk.capability_manager()
        .register_remote_device(DeviceCapabilities {
            device_id: carol_id,
            device_type: DeviceType::Smartphone,
            device_name: "Carol Phone".to_string(),
            supported_channels: HashSet::from([ChannelType::Lan]),
            battery_level: Some(70),
            is_charging: false,
            data_cost_sensitive: false,
        });

    // 3. Create Group
    log::info!("Creating group with Bob and Carol...");
    let group_members = vec![bob_id, carol_id];
    let group_id = sdk
        .create_group("Project Team".to_string(), group_members)
        .await?;
    log::info!("Group created with ID: {}", group_id);

    // 4. Send Group Message
    log::info!("Sending group message: 'Hello Team!'");
    sdk.send_to_group(group_id, MessagePayload::Text("Hello Team!".to_string()))
        .await?;

    sleep(Duration::from_millis(200)).await;

    // 5. Rotate Group Key (Security maintenance)
    log::info!("Rotating group key for enhanced security...");
    sdk.rotate_group_key(group_id).await?;
    log::info!("Group key rotated.");

    // 6. Add new member (Dave)
    let dave_id = DeviceId::new();
    log::info!("Adding Dave to the group...");
    sdk.group_manager().add_member(group_id, dave_id).await?;
    log::info!("Dave added.");

    // 7. Send another group message
    log::info!("Sending group message: 'Welcome Dave!'");
    sdk.send_to_group(group_id, MessagePayload::Text("Welcome Dave!".to_string()))
        .await?;

    sleep(Duration::from_millis(200)).await;

    log::info!("Group chat demo completed.");
    Ok(())
}
