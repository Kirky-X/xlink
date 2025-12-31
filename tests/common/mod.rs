#![allow(dead_code)]
//! Common test utilities and helpers
//!
//! This module provides shared test infrastructure for all test suites,
//! including real implementations, test data generators, and assertion helpers.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use uuid::Uuid;

// use xlink::router::types::{RoutingStrategy, Target}; // These types don't exist in the codebase
use std::collections::HashSet;
use xlink::capability::manager::CapabilityManager;
use xlink::core::error::Result;
use xlink::core::traits::{Channel as ChannelTrait, MessageHandler};
use xlink::core::types::{
    ChannelType, DeviceCapabilities, DeviceId, DeviceType, Message, MessagePayload, NetworkType,
};
use xlink::XLink;

// We need these imports for the TestSdkBuilder

use xlink::channels::memory::MemoryChannel;

// Define NoOpMessageHandler for testing
pub struct NoOpMessageHandler;
#[async_trait::async_trait]
impl MessageHandler for NoOpMessageHandler {
    async fn handle_message(&self, _message: Message) -> Result<()> {
        Ok(())
    }
}

// ==================== Test Data Generators ====================

/// Generate a test device ID
pub fn test_device_id() -> DeviceId {
    DeviceId(Uuid::new_v4())
}

/// Generate multiple test device IDs
pub fn test_device_ids(count: usize) -> Vec<DeviceId> {
    (0..count).map(|_| test_device_id()).collect()
}

/// Create a test message with text payload
pub fn test_text_message(content: &str) -> Message {
    Message::new(
        test_device_id(),
        test_device_id(),
        MessagePayload::Text(content.to_string()),
    )
}

/// Create a test message with binary payload
pub fn test_binary_message(size: usize) -> Message {
    let data = vec![0u8; size];
    Message::new(
        test_device_id(),
        test_device_id(),
        MessagePayload::Binary(data),
    )
}

/// Create test device capabilities
pub fn test_device_capabilities() -> DeviceCapabilities {
    DeviceCapabilities {
        device_id: test_device_id(),
        device_type: DeviceType::Smartphone,
        device_name: "Test Device".to_string(),
        supported_channels: HashSet::from([ChannelType::Lan, ChannelType::BluetoothLE]),
        battery_level: Some(80),
        is_charging: false,
        data_cost_sensitive: true,
    }
}

/// Create device capabilities with specific battery level
pub fn test_device_with_battery(battery_level: u8, is_charging: bool) -> DeviceCapabilities {
    DeviceCapabilities {
        device_id: test_device_id(),
        device_type: DeviceType::Smartphone,
        device_name: "Test Device".to_string(),
        supported_channels: HashSet::from([ChannelType::Lan, ChannelType::BluetoothLE]),
        battery_level: Some(battery_level),
        is_charging,
        data_cost_sensitive: true,
    }
}

/// Create device capabilities with specific network type
pub fn test_device_with_network(_network_type: NetworkType) -> DeviceCapabilities {
    DeviceCapabilities {
        device_id: test_device_id(),
        device_type: DeviceType::Smartphone,
        device_name: "Test Device".to_string(),
        supported_channels: HashSet::from([ChannelType::Lan, ChannelType::Internet]),
        battery_level: Some(80),
        is_charging: false,
        data_cost_sensitive: true,
    }
}

/// Create a test CapabilityManager
pub fn create_test_cap_manager() -> Arc<CapabilityManager> {
    let caps = DeviceCapabilities {
        device_id: test_device_id(),
        device_type: DeviceType::Smartphone,
        device_name: "Test Device".to_string(),
        supported_channels: HashSet::new(),
        battery_level: Some(100),
        is_charging: true,
        data_cost_sensitive: false,
    };
    Arc::new(CapabilityManager::new(caps))
}

// ==================== Real Implementations ====================

// ==================== Test SDK Builder ====================

/// Builder for creating test SDK instances
pub struct TestSdkBuilder {
    device_capabilities: DeviceCapabilities,
    channels: Vec<Arc<dyn ChannelTrait>>,
    network_simulator: Arc<Mutex<Option<NetworkSimulator>>>,
    storage_path: Option<String>,
}

impl TestSdkBuilder {
    pub fn new() -> Self {
        Self {
            device_capabilities: test_device_capabilities(),
            channels: vec![],
            network_simulator: Arc::new(Mutex::new(None)),
            storage_path: None,
        }
    }

    pub fn with_device_capabilities(mut self, capabilities: DeviceCapabilities) -> Self {
        self.device_capabilities = capabilities;
        self
    }

    pub fn with_channel(mut self, channel: Arc<dyn ChannelTrait>) -> Self {
        self.channels.push(channel);
        self
    }

    pub fn with_network_simulator(self, simulator: NetworkSimulator) -> Self {
        if let Ok(mut guard) = self.network_simulator.try_lock() {
            *guard = Some(simulator);
        }
        self
    }

    pub fn with_storage_path(mut self, path: String) -> Self {
        self.storage_path = Some(path);
        self
    }

    pub fn with_low_battery_mode(self, _enabled: bool) -> Self {
        // This would configure the battery monitor
        self
    }

    pub async fn build(self) -> Result<XLink> {
        let mut channels = self.channels;

        // Add a default MemoryChannel if no channels are provided
        if channels.is_empty() {
            let memory_channel = Arc::new(xlink::channels::memory::MemoryChannel::new(
                Arc::new(NoOpMessageHandler),
                10,
            ));
            channels.push(memory_channel);
        }

        let sdk = if let Some(storage_path) = self.storage_path {
            XLink::with_storage_path(self.device_capabilities, channels, storage_path).await?
        } else {
            XLink::new(self.device_capabilities, channels).await?
        };

        // Note: We would need to expose routing strategy setting in the actual SDK
        // For now, this is a placeholder

        Ok(sdk)
    }
}

impl Default for TestSdkBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Assertion Helpers ====================

/// Assert that a message was sent through a specific channel
pub async fn assert_message_sent_through_channel(
    channel: &MemoryChannel,
    expected_recipient: DeviceId,
    expected_payload: &MessagePayload,
) {
    let messages = channel.get_sent_messages().await;
    assert!(
        !messages.is_empty(),
        "No messages were sent through the channel"
    );

    let found = messages
        .iter()
        .any(|msg| msg.recipient == expected_recipient && &msg.payload == expected_payload);

    assert!(
        found,
        "Message not found in sent messages. Expected recipient: {:?}, payload: {:?}",
        expected_recipient, expected_payload
    );
}

/// Assert that a message was NOT sent through a specific channel
pub async fn assert_message_not_sent_through_channel(
    channel: &MemoryChannel,
    expected_recipient: DeviceId,
    expected_payload: &MessagePayload,
) {
    let messages = channel.get_sent_messages().await;

    let found = messages
        .iter()
        .any(|msg| msg.recipient == expected_recipient && &msg.payload == expected_payload);

    assert!(
        !found,
        "Message should not have been sent through the channel. Found: {:?}",
        messages
            .iter()
            .find(|msg| msg.recipient == expected_recipient)
    );
}

/// Assert that two device capability sets are equivalent
pub fn assert_device_capabilities_eq(actual: &DeviceCapabilities, expected: &DeviceCapabilities) {
    assert_eq!(actual.device_id, expected.device_id);
    assert_eq!(actual.supported_channels, expected.supported_channels);
    assert_eq!(actual.battery_level, expected.battery_level);
    assert_eq!(actual.is_charging, expected.is_charging);
}

// ==================== Performance Testing Helpers ====================

/// Measure the execution time of an async operation
pub async fn measure_time<F, Fut, R>(f: F) -> (R, Duration)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let start = std::time::Instant::now();
    let result = f().await;
    let duration = start.elapsed();
    (result, duration)
}

/// Run an operation multiple times and collect timing statistics
pub async fn benchmark_operation<F, Fut, R>(mut operation: F, iterations: usize) -> BenchmarkResult
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let mut times = Vec::with_capacity(iterations);

    for _ in 0..iterations {
        let (_, duration) = measure_time(&mut operation).await;
        times.push(duration);
    }

    times.sort();

    BenchmarkResult {
        min: times[0],
        max: times[times.len() - 1],
        mean: times.iter().sum::<Duration>() / iterations as u32,
        median: times[times.len() / 2],
        p95: times[(times.len() as f64 * 0.95) as usize],
        p99: times[(times.len() as f64 * 0.99) as usize],
    }
}

#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub min: Duration,
    pub max: Duration,
    pub mean: Duration,
    pub median: Duration,
    pub p95: Duration,
    pub p99: Duration,
}

// ==================== Network Simulation ====================

/// Simulate network conditions for testing
pub struct NetworkSimulator {
    packet_loss_rate: f64,
    latency_range: (Duration, Duration),
    bandwidth_limit: Option<u64>, // bytes per second
}

impl NetworkSimulator {
    pub fn perfect() -> Self {
        Self {
            packet_loss_rate: 0.0,
            latency_range: (Duration::from_millis(1), Duration::from_millis(5)),
            bandwidth_limit: None,
        }
    }

    pub fn wifi() -> Self {
        Self {
            packet_loss_rate: 0.01,
            latency_range: (Duration::from_millis(10), Duration::from_millis(50)),
            bandwidth_limit: Some(10_000_000), // 10 Mbps
        }
    }

    pub fn mobile_4g() -> Self {
        Self {
            packet_loss_rate: 0.05,
            latency_range: (Duration::from_millis(50), Duration::from_millis(200)),
            bandwidth_limit: Some(1_000_000), // 1 Mbps
        }
    }

    pub fn poor_network() -> Self {
        Self {
            packet_loss_rate: 0.2,
            latency_range: (Duration::from_millis(200), Duration::from_millis(1000)),
            bandwidth_limit: Some(100_000), // 100 Kbps
        }
    }

    pub async fn simulate_send(&self, data_size: usize) -> Result<()> {
        // Simulate latency
        let latency = if self.latency_range.0 == self.latency_range.1 {
            self.latency_range.0
        } else {
            let range = self.latency_range.1 - self.latency_range.0;
            let random_part =
                Duration::from_millis(rand::random::<u64>() % range.as_millis() as u64);
            self.latency_range.0 + random_part
        };

        tokio::time::sleep(latency).await;

        // Simulate packet loss
        if rand::random::<f64>() < self.packet_loss_rate {
            return Err(xlink::core::error::XLinkError::channel_disconnected(
                "Simulated packet loss".to_string(),
                file!(),
            ));
        }

        // Simulate bandwidth limitation
        if let Some(bandwidth) = self.bandwidth_limit {
            let transfer_time = Duration::from_secs_f64(data_size as f64 / bandwidth as f64);
            tokio::time::sleep(transfer_time).await;
        }

        Ok(())
    }

    // Simulated network behavior
    pub async fn simulate_network_condition(
        &self,
        delay: Duration,
        failure_rate: f64,
    ) -> Result<()> {
        if failure_rate > 0.0 && rand::random::<f64>() < failure_rate {
            return Err(xlink::core::error::XLinkError::channel_disconnected(
                "Simulated network failure".to_string(),
                file!(),
            ));
        }
        tokio::time::sleep(delay).await;
        Ok(())
    }
}

// ==================== Test Environment Setup ====================

/// Set up a complete test environment with multiple devices
pub struct TestEnvironment {
    pub devices: Vec<Arc<XLink>>,
    pub network_simulator: NetworkSimulator,
}

impl TestEnvironment {
    pub async fn new(device_count: usize) -> Result<Self> {
        let mut devices = Vec::with_capacity(device_count);

        for i in 0..device_count {
            let capabilities = DeviceCapabilities {
                device_id: DeviceId(Uuid::new_v4()),
                device_type: DeviceType::Smartphone,
                device_name: format!("Device {}", i),
                supported_channels: std::collections::HashSet::from([ChannelType::Lan]),
                battery_level: Some(80),
                is_charging: false,
                data_cost_sensitive: false,
            };

            let handler = Arc::new(NoOpMessageHandler);
            let channel =
                Arc::new(MemoryChannel::new(handler, 10).with_type(ChannelType::BluetoothLE));
            let sdk = XLink::new(capabilities, vec![channel]).await?;
            devices.push(Arc::new(sdk));
        }

        Ok(Self {
            devices,
            network_simulator: NetworkSimulator::perfect(),
        })
    }

    pub async fn start_all(&self) -> Result<()> {
        for device in &self.devices {
            device.start().await?;
        }
        Ok(())
    }

    pub fn get_device(&self, index: usize) -> Option<Arc<XLink>> {
        self.devices.get(index).cloned()
    }

    pub fn find_device_by_id(&self, device_id: &DeviceId) -> Option<Arc<XLink>> {
        self.devices
            .iter()
            .find(|device| device.device_id() == *device_id)
            .cloned()
    }
}

// ==================== Test Cleanup ====================

/// Clean up test resources
pub async fn cleanup_test_environment(env: TestEnvironment) {
    // Add any cleanup logic here
    drop(env);
}

/// Reset global test state
pub fn reset_test_state() {
    // Reset any global state that might affect tests
    // This is a placeholder for any global cleanup needed
}

/// Establish cryptographic sessions between devices for group communication
pub async fn establish_device_sessions(devices: &[&xlink::XLink]) -> Result<()> {
    // Register each device's public key with every other device's group manager
    for i in 0..devices.len() {
        for j in 0..devices.len() {
            if i != j {
                let device_id = devices[j].device_id();
                let public_key = devices[j].public_key();
                devices[i].register_device_key(device_id, public_key)?;

                // Also register channel state for LAN channel (which is the default in TestSdkBuilder)
                let channel_state = xlink::core::types::ChannelState {
                    available: true,
                    rtt_ms: 10,
                    jitter_ms: 0,
                    packet_loss_rate: 0.0,
                    bandwidth_bps: 1000000,
                    signal_strength: Some(100),
                    network_type: xlink::core::types::NetworkType::Unknown,
                    failure_count: 0,
                    last_heartbeat: 0,
                    distance_meters: Some(10.0),
                };

                // Update channel state for both directions
                devices[i].capability_manager().update_channel_state(
                    device_id,
                    xlink::core::types::ChannelType::Lan,
                    channel_state.clone(),
                );
                devices[j].capability_manager().update_channel_state(
                    devices[i].device_id(),
                    xlink::core::types::ChannelType::Lan,
                    channel_state,
                );
            }
        }
    }
    Ok(())
}
