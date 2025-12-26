use crate::capability::manager::CapabilityManager;
use crate::core::types::{
    ChannelState, ChannelType, DeviceCapabilities, DeviceId, DeviceType, NetworkType,
};
use mdns_sd::{ServiceDaemon, ServiceEvent};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct DiscoveryInfo {
    _device_id: DeviceId,
    _first_seen: Instant,
    _last_seen: Instant,
    _rssi: Option<i8>,
    _distance_meters: Option<f32>,
    _discovery_method: DiscoveryMethod,
}

#[derive(Debug, Clone)]
enum DiscoveryMethod {
    Mdns,
    #[allow(dead_code)]
    BleScan,
}

pub struct DiscoveryManager {
    cap_manager: Arc<CapabilityManager>,
    mdns_task: Option<JoinHandle<()>>,
    ble_task: Option<JoinHandle<()>>,
    discovery_cache: Arc<tokio::sync::RwLock<std::collections::HashMap<DeviceId, DiscoveryInfo>>>,
    _start_time: Instant,
}

impl DiscoveryManager {
    pub fn new(cap_manager: Arc<CapabilityManager>) -> Self {
        Self {
            cap_manager,
            mdns_task: None,
            ble_task: None,
            discovery_cache: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            _start_time: Instant::now(),
        }
    }

    pub async fn start_discovery(&mut self) -> (Option<JoinHandle<()>>, Option<JoinHandle<()>>) {
        let cap_manager = self.cap_manager.clone();
        let discovery_cache = self.discovery_cache.clone();

        let mdns_task = tokio::spawn(async move {
            log::info!("Starting mDNS discovery with <5s target...");

            let mdns = match ServiceDaemon::new() {
                Ok(d) => d,
                Err(e) => {
                    log::error!("Failed to create mDNS daemon: {}", e);
                    return;
                }
            };
            let service_type = "_xpush._tcp.local.";

            let receiver = match mdns.browse(service_type) {
                Ok(r) => r,
                Err(e) => {
                    log::error!("Failed to browse mDNS: {}", e);
                    return;
                }
            };

            let start_time = Instant::now();
            let discovery_timeout = Duration::from_secs(5);

            while let Ok(event) = receiver.recv_timeout(discovery_timeout) {
                if let ServiceEvent::ServiceResolved(info) = event {
                    if !Self::filter_service(&info) {
                        continue;
                    }

                    log::info!(
                        "mDNS Resolved: {} ({}ms)",
                        info.get_fullname(),
                        start_time.elapsed().as_millis()
                    );

                    let fingerprint = Self::identify_fingerprint(&info);
                    log::debug!("Device fingerprint: {}", fingerprint);

                    if let Some(device_id_str) = info.get_property_val_str("id") {
                        if let Ok(device_id_uuid) = Uuid::parse_str(device_id_str) {
                            let device_id = DeviceId(device_id_uuid);

                            let caps = DeviceCapabilities {
                                device_id,
                                device_type: match info.get_property_val_str("type") {
                                    Some("mobile") => DeviceType::Smartphone,
                                    Some("desktop") => DeviceType::Desktop,
                                    _ => DeviceType::IoTDevice,
                                },
                                device_name: info.get_hostname().to_string(),
                                supported_channels: HashSet::new(),
                                battery_level: info
                                    .get_property_val_str("bat")
                                    .and_then(|v| v.parse().ok()),
                                is_charging: false,
                                data_cost_sensitive: false,
                            };

                            cap_manager.register_remote_device(caps);

                            let distance = Self::estimate_distance_from_network(&info);
                            let state = ChannelState {
                                available: true,
                                rtt_ms: start_time.elapsed().as_millis() as u32,
                                failure_count: 0,
                                last_heartbeat: 0,
                                signal_strength: None,
                                distance_meters: Some(distance),
                                network_type: NetworkType::WiFi,
                                bandwidth_bps: 0,
                                jitter_ms: 0,
                                packet_loss_rate: 0.0,
                            };
                            cap_manager.update_channel_state(
                                device_id,
                                ChannelType::Internet,
                                state,
                            );

                            let mut cache = discovery_cache.write().await;
                            cache.insert(
                                device_id,
                                DiscoveryInfo {
                                    _device_id: device_id,
                                    _first_seen: Instant::now(),
                                    _last_seen: Instant::now(),
                                    _rssi: None,
                                    _distance_meters: Some(distance),
                                    _discovery_method: DiscoveryMethod::Mdns,
                                },
                            );
                        }
                    }
                }
            }
        });

        let ble_task = tokio::spawn(async move {
            log::info!("BLE scanning simulation - would scan for 5 seconds");
            tokio::time::sleep(Duration::from_secs(5)).await;
            log::info!("BLE discovery completed");
        });

        self.mdns_task = None;
        self.ble_task = None;
        (Some(mdns_task), Some(ble_task))
    }

    pub async fn stop_discovery(&self) {
        log::info!("DiscoveryManager stop called (tasks managed by SDK)");
    }

    pub async fn clear_cache(&self) {
        let mut cache = self.discovery_cache.write().await;
        cache.clear();
    }

    pub async fn simulate_background_discovery(
        &self,
        _device_id: DeviceId,
    ) -> crate::core::error::Result<()> {
        log::info!("Simulating background discovery for device...");
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    fn identify_fingerprint(info: &mdns_sd::ServiceInfo) -> String {
        let mut fingerprint = String::new();
        fingerprint.push_str(info.get_hostname());

        if let Some(model) = info.get_property_val_str("model") {
            fingerprint.push('|');
            fingerprint.push_str(model);
        }

        if let Some(os) = info.get_property_val_str("os") {
            fingerprint.push('|');
            fingerprint.push_str(os);
        }

        fingerprint
    }

    fn filter_service(info: &mdns_sd::ServiceInfo) -> bool {
        info.get_fullname().contains("_xpush._tcp.local.")
    }

    fn estimate_distance_from_network(_info: &mdns_sd::ServiceInfo) -> f32 {
        15.0
    }
}
