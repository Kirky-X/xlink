use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use xpush::channels::memory::MemoryChannel;
use xpush::core::types::{
    ChannelState, ChannelType, DeviceCapabilities, DeviceId, DeviceType, MessagePayload,
};
use xpush::UnifiedPushSDK;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // 1. Setup Device A (Alice)
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

    // 2. Setup Device B (Bob)
    let bob_id = DeviceId::new();
    // Bob is just a target ID for this demo, we don't spin up a full SDK for him
    // in this single process unless we mock the network fully.
    // For simplicity, we assume the MemoryChannel "delivers" it.

    // 3. Create SDK for Alice
    // We need a channel. For the demo, we create a MemoryChannel.
    // The handler is created inside SDK usually, but for circular dep in construction
    // we often use a factory or lazy init. Here we do a slight workaround for the demo.

    // Placeholder handler for construction
    let (_tx, _rx) = tokio::sync::mpsc::channel::<()>(1);

    // Construct Alice's SDK
    // We need to construct the channel first, but the channel needs a handler.
    // In a real app, the Channel impl might have an internal mpsc, and the SDK polls it.
    // Here we use the SDK's handler logic.

    // Let's create the SDK with an empty channel list first, then inject (if mutable)
    // or just pass a channel that we wire up.
    // To keep it simple and runnable:

    // We will use a channel that just logs for now, as we can't easily wire the handler back
    // into the constructor without more complex Arc<Mutex<Option<...>>> logic.

    struct DemoHandler;
    #[async_trait::async_trait]
    impl xpush::core::traits::MessageHandler for DemoHandler {
        async fn handle_message(
            &self,
            _msg: xpush::core::types::Message,
        ) -> xpush::core::error::Result<()> {
            Ok(())
        }
    }

    let mem_channel = Arc::new(MemoryChannel::new(Arc::new(DemoHandler), 50)); // 50ms latency

    let sdk = UnifiedPushSDK::new(alice_caps, vec![mem_channel.clone()]).await?;
    sdk.start().await?;

    // 4. Register Bob's capabilities in Alice's manager so routing works
    // In real life, this happens via Discovery
    let bob_caps = DeviceCapabilities {
        device_id: bob_id,
        device_type: DeviceType::Laptop,
        device_name: "Bob Laptop".to_string(),
        supported_channels: HashSet::from([ChannelType::Lan]),
        battery_level: Some(100),
        is_charging: true,
        data_cost_sensitive: false,
    };
    sdk.capability_manager().register_remote_device(bob_caps);

    // Update channel state for Bob (Simulate discovery found a good link)
    sdk.capability_manager().update_channel_state(
        bob_id,
        ChannelType::Lan,
        ChannelState {
            available: true,
            rtt_ms: 20, // Fast
            jitter_ms: 5,
            packet_loss_rate: 0.0,
            bandwidth_bps: 1024 * 1024,
            signal_strength: Some(-40),
            network_type: xpush::core::types::NetworkType::WiFi,
            failure_count: 0,
            last_heartbeat: std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            distance_meters: Some(10.0), // 10 meters away
        },
    );

    log::info!("--- System Initialized ---");
    log::info!("Alice: {}", alice_id);
    log::info!("Bob:   {}", bob_id);

    // 5. Send Message
    log::info!("Sending 'Hello Bob'...");
    sdk.send(bob_id, MessagePayload::Text("Hello Bob!".to_string()))
        .await?;

    // Wait for async operations
    sleep(Duration::from_millis(200)).await;

    // 6. Simulate High Priority Message
    log::info!("Sending Critical Alert...");
    // Note: In a full impl, we'd set priority on the message builder.
    // The current simple API defaults to Normal.
    // Let's assume we added a builder or method for it.
    sdk.send(bob_id, MessagePayload::Text("FIRE ALARM!".to_string()))
        .await?;

    sleep(Duration::from_millis(200)).await;

    log::info!("Demo completed successfully.");
    Ok(())
}
