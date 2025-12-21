# XPush SDK 用户指南 (User Guide)

欢迎使用 XPush SDK，这是一个高性能、安全且具备多通道自适应能力的统一推送与通信框架。

## 1. 核心概念

*   **DeviceId**: 设备的唯一标识（UUID）。
*   **Channel**: 消息传输通道（如 BLE, LAN, Internet, Mesh）。
*   **Router**: 智能路由引擎，根据延迟、带宽、电量和成本自动选择最优通道。
*   **Group**: 支持 TreeKEM 加密的端到端加密群组。

## 2. 快速入门 (Rust)

### 2.1 初始化 SDK

```rust
use xpush::UnifiedPushSDK;
use xpush::core::types::{DeviceCapabilities, DeviceType, DeviceId};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let config = DeviceCapabilities {
        device_id: DeviceId::new(),
        device_type: DeviceType::Smartphone,
        device_name: "MyDevice".to_string(),
        supported_channels: Default::default(), // 默认支持所有可用通道
        battery_level: Some(100),
        is_charging: true,
        data_cost_sensitive: false,
    };

    // 初始化通道（示例使用内存通道）
    let channels = vec![]; 
    
    let sdk = UnifiedPushSDK::new(config, channels).await.unwrap();
    sdk.start().await.unwrap();
}
```

### 2.2 发送消息

```rust
use xpush::core::types::{DeviceId, MessagePayload};

let target = DeviceId::new();
sdk.send(target, MessagePayload::Text("Hello XPush!".to_string())).await.unwrap();
```

### 2.3 接收消息

```rust
while let Some(msg) = sdk.receive().await {
    println!("Received message: {:?}", msg.payload);
}
```

## 3. 高级特性

### 3.1 群组通信

XPush 支持加密群组：

```rust
let members = vec![target_id];
let group_id = sdk.create_group("Project X".to_string(), members).await.unwrap();

sdk.send_to_group(group_id, MessagePayload::Text("Group message".to_string())).await.unwrap();
```

### 3.2 功耗感知

SDK 会自动根据 `battery_level` 调整路由策略。当电量低于 20% 时，SDK 会优先选择低功耗通道（如 BLE），并降低心跳频率。

### 3.3 Mesh 中继

在网络受限环境下，SDK 会尝试通过邻近节点进行 Mesh 中继转发，以提高消息送达率。

## 4. C 语言调用示例

```c
#include "xpush.h"

int main() {
    xpush_sdk_t* sdk = xpush_init();
    
    xpush_device_id_t target = { /* ... */ };
    xpush_send_text(sdk, target, "Hello from C!");
    
    xpush_free(sdk);
    return 0;
}
```

## 5. 故障排查

*   **NoRouteFound**: 检查目标设备是否在线，或是否已交换通道状态信息。
*   **CryptoError**: 检查群组公钥是否已正确注册。
