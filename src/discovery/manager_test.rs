use crate::capability::manager::CapabilityManager;
use crate::core::types::{ChannelState, ChannelType, DeviceCapabilities, DeviceId, DeviceType, NetworkType};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use uuid::Uuid;

// Test version of discovery manager without external dependencies
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DiscoveryInfo {
    device_id: DeviceId,
    first_seen: Instant,
    last_seen: Instant,
    rssi: Option<i8>, // BLE 信号强度
    distance_meters: Option<f32>, // 估算距离
    discovery_method: DiscoveryMethod,
}

#[derive(Debug, Clone)]
enum DiscoveryMethod {
    BleScan,
    Mdns,
}

// Test version of discovery manager - no external dependencies
pub struct DiscoveryManager {
    cap_manager: Arc<CapabilityManager>,
    mdns_task: Option<JoinHandle<()>>,
    ble_task: Option<JoinHandle<()>>,
    discovery_cache: Arc<tokio::sync::RwLock<std::collections::HashMap<DeviceId, DiscoveryInfo>>>,
    start_time: Instant,
}

impl DiscoveryManager {
    pub fn new(cap_manager: Arc<CapabilityManager>) -> Self {
        Self {
            cap_manager,
            mdns_task: None,
            ble_task: None,
            discovery_cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            start_time: Instant::now(),
        }
    }

    pub fn start_discovery(&mut self) {
        if self.mdns_task.is_some() || self.ble_task.is_some() { return; }

        let cap_manager = self.cap_manager.clone();
        let discovery_cache = self.discovery_cache.clone();
        
        // Test version: Simulate mDNS discovery
        self.mdns_task = Some(tokio::spawn(async move {
            log::info!("Starting simulated mDNS discovery with <5s target...");
            
            let start_time = Instant::now();
            let discovery_timeout = Duration::from_secs(5);
            
            // Simulate discovering 3 devices
            for i in 0..3 {
                if start_time.elapsed() > discovery_timeout {
                    break;
                }
                
                let device_id = DeviceId(Uuid::new_v4());
                let distance_estimate = 5.0 + (i as f32 * 5.0); // 5-15 meters
                
                // Update discovery cache
                let mut cache = discovery_cache.write().await;
                cache.insert(device_id, DiscoveryInfo {
                    device_id,
                    first_seen: Instant::now(),
                    last_seen: Instant::now(),
                    rssi: None,
                    distance_meters: Some(distance_estimate),
                    discovery_method: DiscoveryMethod::Mdns,
                });
                drop(cache);
                
                // Build capability object
                let caps = DeviceCapabilities {
                    device_id,
                    device_type: DeviceType::Laptop,
                    device_name: format!("test_device_{}", i),
                    supported_channels: HashSet::from([ChannelType::Lan]),
                    battery_level: None,
                    is_charging: true,
                    data_cost_sensitive: false,
                };
                
                cap_manager.register_remote_device(caps);
                
                // Update channel state
                cap_manager.update_channel_state(
                    device_id,
                    ChannelType::Lan,
                    ChannelState {
                        available: true,
                        rtt_ms: 5,
                        network_type: NetworkType::WiFi,
                        bandwidth_bps: 100_000_000,
                        signal_strength: Some(-50),
                        ..Default::default()
                    }
                );
                
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            
            log::info!("Simulated mDNS discovery completed in {}ms", start_time.elapsed().as_millis());
        }));

        // Test version: Simulate BLE discovery
        let cap_manager_ble = self.cap_manager.clone();
        let discovery_cache_ble = self.discovery_cache.clone();
        
        self.ble_task = Some(tokio::spawn(async move {
            log::info!("Starting simulated BLE discovery for 10m range...");
            
            let start_time = Instant::now();
            let ble_timeout = Duration::from_secs(5);
            
            // Simulate discovering 2 BLE devices
            for i in 0..2 {
                if start_time.elapsed() > ble_timeout {
                    break;
                }
                
                let device_id = DeviceId(Uuid::new_v4());
                let rssi = -60 - (i as i8 * 10); // -60, -70 dBm
                let distance = estimate_ble_distance(rssi);
                
                if distance <= 10.0 {
                    let mut cache = discovery_cache_ble.write().await;
                    cache.insert(device_id, DiscoveryInfo {
                        device_id,
                        first_seen: Instant::now(),
                        last_seen: Instant::now(),
                        rssi: Some(rssi),
                        distance_meters: Some(distance),
                        discovery_method: DiscoveryMethod::BleScan,
                    });
                    drop(cache);
                    
                    // Register BLE device
                    let caps = DeviceCapabilities {
                        device_id,
                        device_type: DeviceType::Smartphone,
                        device_name: format!("ble_device_{}", i),
                        supported_channels: HashSet::from([ChannelType::BluetoothLE]),
                        battery_level: None,
                        is_charging: true,
                        data_cost_sensitive: false,
                    };
                    
                    cap_manager_ble.register_remote_device(caps);
                    
                    // Update BLE channel state
                    cap_manager_ble.update_channel_state(
                        device_id,
                        ChannelType::BluetoothLE,
                        ChannelState {
                            available: true,
                            rtt_ms: 20,
                            network_type: NetworkType::Bluetooth,
                            bandwidth_bps: 1_000_000,
                            signal_strength: Some(rssi),
                            ..Default::default()
                        }
                    );
                }
                
                tokio::time::sleep(Duration::from_millis(700)).await;
            }
            
            log::info!("Simulated BLE discovery completed in {}ms", start_time.elapsed().as_millis());
        }));
    }

    pub fn stop_discovery(&mut self) {
        if let Some(task) = self.mdns_task.take() {
            task.abort();
        }
        if let Some(task) = self.ble_task.take() {
            task.abort();
        }
    }
    
    // Get discovery statistics
    pub async fn get_discovery_stats(&self) -> DiscoveryStats {
        let cache = self.discovery_cache.read().await;
        let elapsed = self.start_time.elapsed();
        
        DiscoveryStats {
            total_devices: cache.len(),
            ble_devices: cache.values().filter(|info| matches!(info.discovery_method, DiscoveryMethod::BleScan)).count(),
            mdns_devices: cache.values().filter(|info| matches!(info.discovery_method, DiscoveryMethod::Mdns)).count(),
            average_distance: cache.values()
                .filter_map(|info| info.distance_meters)
                .sum::<f32>() / cache.len().max(1) as f32,
            discovery_time_ms: elapsed.as_millis() as u64,
        }
    }

    pub async fn simulate_background_discovery(&self, device_id: DeviceId) -> crate::core::error::Result<()> {
        log::info!("Simulating background discovery for device: {}", device_id);
        
        // 模拟后台发现一个新设备
        let caps = DeviceCapabilities {
            device_id,
            device_type: DeviceType::Smartphone,
            device_name: format!("BackgroundDevice-{}", device_id),
            supported_channels: [ChannelType::BluetoothLE, ChannelType::Lan].into_iter().collect(),
            battery_level: Some(85),
            is_charging: false,
            data_cost_sensitive: false,
        };

        self.cap_manager.register_remote_device(caps);
        
        // 模拟系统通知
        log::info!("NOTIFY: New device discovered in background: {}", device_id);
        
        Ok(())
    }
}

// Discovery statistics
#[derive(Debug, Clone)]
pub struct DiscoveryStats {
    pub total_devices: usize,
    pub ble_devices: usize,
    pub mdns_devices: usize,
    pub average_distance: f32,
    pub discovery_time_ms: u64,
}

// Estimate BLE distance based on RSSI
fn estimate_ble_distance(rssi: i8) -> f32 {
    // Simplified distance estimation based on RSSI
    // This is a rough approximation for testing purposes
    if rssi > -50 {
        2.0 // Very close
    } else if rssi > -60 {
        5.0 // Close
    } else if rssi > -70 {
        8.0 // Medium distance
    } else if rssi > -80 {
        12.0 // Far
    } else {
        20.0 // Very far
    }
}