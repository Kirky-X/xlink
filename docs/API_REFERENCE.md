<div align="center">

# 📘 API Reference

### Complete API Documentation

[🏠 Home](../README.md) • [📖 User Guide](USER_GUIDE.md) • [🏗️ Architecture](ARCHITECTURE.md)

---

</div>

## 📋 Table of Contents

- [Overview](#overview)
- [Core API](#core-api)
  - [Initialization](#initialization)
  - [Configuration](#configuration)
  - [Cipher Operations](#cipher-operations)
  - [Key Management](#key-management)
- [Algorithms](#algorithms)
- [Error Handling](#error-handling)
- [Type Definitions](#type-definitions)
- [Examples](#examples)

---

## Overview

<div align="center">

### 🎯 API Design Principles

</div>

<table>
<tr>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/easy.png" width="64"><br>
<b>Simple</b><br>
Intuitive and easy to use
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/security-checked.png" width="64"><br>
<b>Safe</b><br>
Type-safe and secure by default
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/module.png" width="64"><br>
<b>Composable</b><br>
Build complex workflows easily
</td>
<td width="25%" align="center">
<img src="https://img.icons8.com/fluency/96/000000/documentation.png" width="64"><br>
<b>Well-documented</b><br>
Comprehensive documentation
</td>
</tr>
</table>

---

## Core API

### Initialization

<div align="center">

#### 🚀 Getting Started

</div>

---

#### `XLink::new()`

Create a new xlink SDK instance with device capabilities and channels.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub async fn new(
    config: DeviceCapabilities,
    channels: Vec<Arc<dyn Channel>>
) -> Result<Self, Error>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `config: DeviceCapabilities` - Device configuration including device ID and supported channels
- `channels: Vec<Arc<dyn Channel>>` - List of communication channels to use

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;XLink, Error&gt;</code> - SDK instance on success, Error on failure</td>
</tr>
<tr>
<td><b>Errors</b></td>
<td>

- `Error::InvalidConfig` - Invalid device configuration
- `Error::ChannelError` - Channel initialization failed

</td>
</tr>
</table>

**Example:**

```rust
use xlink::XLink;
use xlink::core::types::DeviceCapabilities;
use xlink::core::types::DeviceId;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device_id = DeviceId::new();
    let capabilities = DeviceCapabilities {
        device_id,
        device_type: xlink::core::types::DeviceType::Smartphone,
        device_name: "My Phone".to_string(),
        supported_channels: HashSet::from([ChannelType::Lan]),
        battery_level: Some(80),
        is_charging: false,
        data_cost_sensitive: false,
    };

    let sdk = XLink::new(capabilities, vec![]).await?;
    println!("✅ SDK created successfully");

    Ok(())
}
```

---

#### `start()`

Start the SDK and begin listening for messages.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub async fn start(&self) -> Result<(), Error>
```

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;(), Error&gt;</code></td>
</tr>
<tr>
<td><b>Errors</b></td>
<td>

- `Error::AlreadyRunning` - SDK already started
- `Error::ChannelError` - Failed to start channels

</td>
</tr>
</table>

**Example:**

```rust
let capabilities = DeviceCapabilities {
    device_id,
    device_type: xlink::core::types::DeviceType::Smartphone,
    device_name: "My Phone".to_string(),
    supported_channels: HashSet::from([ChannelType::Lan]),
    battery_level: Some(80),
    is_charging: false,
    data_cost_sensitive: false,
};

let sdk = XLink::new(capabilities, vec![]).await?;
sdk.start().await?;
println!("✅ SDK is now listening for messages");
```

---

#### `stop()`

Stop the SDK and release all resources.

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub async fn stop(&self)
```

</td>
</tr>
<tr>
<td><b>Description</b></td>
<td>Stops all channels, releases resources, and cleans up the SDK state.</td>
</tr>
</table>

**Example:**

```rust
sdk.start().await?;
// ... use SDK ...
sdk.stop().await;
println!("✅ SDK stopped");
```

---

### Configuration

<div align="center">

#### ⚙️ Device Capabilities

</div>

---

#### `DeviceCapabilities`

Device configuration structure for SDK initialization.

<table>
<tr>
<td width="30%"><b>Type</b></td>
<td width="70%">

```rust
pub struct DeviceCapabilities {
    pub device_id: DeviceId,
    pub device_type: DeviceType,
    pub device_name: String,
    pub supported_channels: HashSet<ChannelType>,
    pub battery_level: Option<u8>,
    pub is_charging: bool,
    pub data_cost_sensitive: bool,
}
```

</td>
</tr>
<tr>
<td><b>Fields</b></td>
<td>

- `device_id: DeviceId` - Unique identifier for this device
- `device_type: DeviceType` - Type of device (Smartphone, Laptop, etc.)
- `device_name: String` - Human-readable device name
- `supported_channels: HashSet<ChannelType>` - Set of supported channel types
- `battery_level: Option<u8>` - Current battery percentage (0-100)
- `is_charging: bool` - Whether the device is currently charging
- `data_cost_sensitive: bool` - Whether to optimize for data usage

</td>
</tr>
</table>

**Example:**

```rust
use xlink::core::types::DeviceCapabilities;
use std::collections::HashSet;
use xlink::core::types::ChannelType;

let capabilities = DeviceCapabilities {
    device_id: DeviceId::new(),
    device_type: DeviceType::Smartphone,
    device_name: "My Phone".to_string(),
    supported_channels: HashSet::from([
        ChannelType::Lan,
        ChannelType::BluetoothLE,
    ]),
    battery_level: Some(80),
    is_charging: false,
    data_cost_sensitive: false,
};
```

---

#### `DeviceId`

Unique device identifier type based on UUID.

<table>
<tr>
<td width="30%"><b>Type</b></td>
<td width="70%">

```rust
pub struct DeviceId(pub Uuid);
```

</td>
</tr>
<tr>
<td><b>Methods</b></td>
<td>

- `new() -> Self` - Create a new random device ID
- `from_str(s: &str) -> Result<Self, uuid::Error>` - Create from UUID string
- `to_string(&self) -> String` - Get UUID string representation

</td>
</tr>
</table>

**Example:**

```rust
use xlink::core::types::DeviceId;

// Create new device ID
let device_id = DeviceId::new();

// Parse from string
let device_id: DeviceId = "550e8400-e29b-41d4-a716-446655440000".parse()?;
```

---

### Cipher Operations

<div align="center">

#### 🔐 Encryption and Decryption

</div>

xlink 使用 `CryptoEngine` 提供端到端加密功能，基于 X25519 密钥交换和 ChaCha20Poly1305 加密算法。

---

#### `CryptoEngine`

核心加密引擎，提供密钥交换、消息加密/解密和数字签名功能。

<table>
<tr>
<td width="30%"><b>Type</b></td>
<td width="70%">

```rust
pub struct CryptoEngine {
    static_secret: StaticSecret,
    public_key: PublicKey,
    signing_key: SigningKey,
    sessions: Arc<DashMap<DeviceId, Mutex<SessionState>>>,
}
```

</td>
</tr>
</table>

**主要功能：**
- **密钥交换**：X25519 Diffie-Hellman 密钥交换
- **消息加密**：ChaCha20Poly1305 AEAD 加密
- **数字签名**：Ed25519 签名验证
- **密钥派生**：HKDF-SHA256 密钥派生

---

#### `CryptoEngine::new()`

创建新的加密引擎实例。

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn new() -> Self
```

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>CryptoEngine</code> - 加密引擎实例</td>
</tr>
</table>

**Example:**

```rust
use xlink::crypto::engine::CryptoEngine;

let crypto = CryptoEngine::new();
let public_key = crypto.public_key();
```

---

#### `CryptoEngine::public_key()`

获取本地设备的 X25519 公钥。

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn public_key(&self) -> PublicKey
```

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>PublicKey</code> - X25519 公钥，用于与远程设备建立安全通道</td>
</tr>
</table>

---

#### `CryptoEngine::verifying_key()`

获取本地设备的 Ed25519 验证公钥，用于数字签名验证。

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn verifying_key(&self) -> VerifyingKey
```

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>VerifyingKey</code> - Ed25519 验证公钥</td>
</tr>
</table>

---

#### `CryptoEngine::encrypt_message()`

使用 ChaCha20Poly1305 加密消息。

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn encrypt_message(
    &self,
    recipient: &DeviceId,
    plaintext: &[u8]
) -> Result<Vec<u8>, Error>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `recipient: &DeviceId` - 接收方设备 ID
- `plaintext: &[u8]` - 要加密的明文数据

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;Vec&lt;u8&gt;, Error&gt;</code> - 加密后的密文</td>
</tr>
<tr>
<td><b>Errors</b></td>
<td>

- `Error::SessionNotFound` - 与接收方的会话不存在
- `Error::EncryptionFailed` - 加密操作失败

</td>
</tr>
</table>

**Example:**

```rust
use xlink::crypto::engine::CryptoEngine;

let crypto = CryptoEngine::new();
let recipient: DeviceId = "peer-device".to_string().try_into()?;

let plaintext = b"Secret message";
let ciphertext = crypto.encrypt_message(&recipient, plaintext)?;
```

<details>
<summary><b>📝 Notes</b></summary>

- 每次加密自动生成随机 Nonce
- 使用 Double Ratchet 算法更新密钥
- 密文包含认证标签，篡改检测

</details>

---

#### `CryptoEngine::decrypt_message()`

解密接收到的消息。

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub fn decrypt_message(
    &self,
    sender: &DeviceId,
    ciphertext: &[u8]
) -> Result<Vec<u8>, Error>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `sender: &DeviceId` - 发送方设备 ID
- `ciphertext: &[u8]` - 要解密的密文数据

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;Vec&lt;u8&gt;, Error&gt;</code> - 解密后的明文</td>
</tr>
<tr>
<td><b>Errors</b></td>
<td>

- `Error::SessionNotFound` - 与发送方的会话不存在
- `Error::DecryptionFailed` - 解密或认证失败
- `Error::InvalidCiphertext` - 密文格式错误

</td>
</tr>
</table>

**Example:**

```rust
use xlink::crypto::engine::CryptoEngine;

let crypto = CryptoEngine::new();
let sender: DeviceId = "peer-device".to_string().try_into()?;

let plaintext = crypto.decrypt_message(&sender, &ciphertext)?;
```

---

#### `CryptoEngine::establish_session()`

与远程设备建立加密会话（执行 X25519 密钥交换）。

<table>
<tr>
<td width="30%"><b>Signature</b></td>
<td width="70%">

```rust
pub async fn establish_session(
    &self,
    peer_device_id: &DeviceId,
    peer_public_key: PublicKey,
    peer_verifying_key: Option<VerifyingKey>
) -> Result<(), Error>
```

</td>
</tr>
<tr>
<td><b>Parameters</b></td>
<td>

- `peer_device_id: &DeviceId` - 远程设备 ID
- `peer_public_key: PublicKey` - 远程设备的 X25519 公钥
- `peer_verifying_key: Option<VerifyingKey>` - 远程设备的 Ed25519 验证公钥（可选）

</td>
</tr>
<tr>
<td><b>Returns</b></td>
<td><code>Result&lt;(), Error&gt;</code></td>
</tr>
<tr>
<td><b>Errors</b></td>
<td>

- `Error::SessionAlreadyExists` - 会话已存在
- `Error::InvalidPeerKey` - 无效的公钥

</td>
</tr>
</table>

**Example:**

```rust
use xlink::crypto::engine::CryptoEngine;

let crypto = CryptoEngine::new();
let peer_device_id: DeviceId = "peer-device".to_string().try_into()?;
let peer_public_key = other_crypto.public_key();

crypto.establish_session(&peer_device_id, peer_public_key, None).await?;
```

<details>
<summary><b>📝 Security Notes</b></summary>

- 密钥交换使用 X25519 DH 算法
- 可选 Ed25519 签名验证防止中间人攻击
- 建立的会话使用 Double Ratchet 算法进行前向保密

</details>

---

### Key Management

<div align="center">

#### 🔑 Session Key Management

</div>

xlink 的密钥管理集成在 `CryptoEngine` 中，自动处理密钥的生命周期。

---

#### 会话状态管理

`CryptoEngine` 自动管理与其他设备的会话密钥：

<table>
<tr>
<td width="30%"><b>方法</b></td>
<td width="70%"><b>描述</b></td>
</tr>
<tr>
<td><code>establish_session()</code></td>
<td>建立与远程设备的安全会话，执行 X25519 密钥交换</td>
</tr>
<tr>
<td><code>encrypt_message()</code></td>
<td>加密消息时自动更新发送链密钥</td>
</tr>
<tr>
<td><code>decrypt_message()</code></td>
<td>解密消息时自动更新接收链密钥</td>
</tr>
<tr>
<td><code>clear_sessions()</code></td>
<td>清除所有会话状态</td>
</tr>
</table>

**Example: 完整的端到端加密流程**

```rust
use xlink::XLink;
use xlink::core::types::{DeviceCapabilities, DeviceId};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device_id = DeviceId::new();
    let capabilities = DeviceCapabilities {
        device_id,
        device_type: DeviceType::Smartphone,
        device_name: "device-001",
        supported_channels: HashSet::from([ChannelType::Lan]),
        battery_level: Some(80),
        is_charging: false,
        data_cost_sensitive: false,
    };

    let sdk = XLink::new(capabilities, vec![]).await?;

    // 1. 启动 SDK
    sdk.start().await?;

    // 2. 建立与远程设备的加密会话
    let peer_id: DeviceId = "peer-device".to_string().parse()?;
    let peer_public_key = /* 从远程设备获取 */;

    sdk.crypto().establish_session(&peer_id, peer_public_key, None).await?;

    // 3. 发送加密消息（SDK 自动加密）
    let payload = /* 消息载荷 */;
    sdk.send(peer_id, payload).await?;

    // 4. 停止 SDK
    sdk.stop().await;

    Ok(())
}
```

<details>
<summary><b>📝 Key Management Features</b></summary>

- **前向保密**：使用 Double Ratchet 算法，每次通信后更新密钥
- **密钥派生**：使用 HKDF-SHA256 从共享秘密派生会话密钥
- **自动重同步**：处理密钥状态同步问题

</details>

---

## Algorithms

<div align="center">

#### 🔐 Supported Cryptographic Algorithms

</div>

xlink 使用现代密码学算法确保通信安全。

### 密钥交换算法

<table>
<tr>
<th>算法</th>
<th>类型</th>
<th>安全级别</th>
<th>描述</th>
</tr>
<tr>
<td><b>X25519</b></td>
<td>椭圆曲线 DH</td>
<td>🟢 高</td>
<td>现代推荐的密钥交换算法，256 位安全级别</td>
</tr>
</table>

### 消息加密算法

<table>
<tr>
<th>算法</th>
<th>类型</th>
<th>安全级别</th>
<th>性能</th>
<th>描述</th>
</tr>
<tr>
<td><b>ChaCha20-Poly1305</b></td>
<td>AEAD</td>
<td>🟢 高</td>
<td>⚡⚡⚡ 快速</td>
<td>现代认证加密，适用于所有平台</td>
</tr>
</table>

### 数字签名算法

<table>
<tr>
<th>算法</th>
<th>密钥大小</th>
<th>安全级别</th>
<th>签名大小</th>
<th>描述</th>
</tr>
<tr>
<td><b>Ed25519</b></td>
<td>256-bit</td>
<td>🟢 高</td>
<td>64 bytes</td>
<td>现代高效的数字签名算法</td>
</tr>
</table>

### 密钥派生算法

<table>
<tr>
<th>算法</th>
<th>类型</th>
<th>安全级别</th>
<th>描述</th>
</tr>
<tr>
<td><b>HKDF-SHA256</b></td>
<td>密钥派生</td>
<td>🟢 高</td>
<td>从共享秘密派生会话密钥</td>
</tr>
</table>

---

## Error Handling

<div align="center">

#### 🚨 Error Types and Handling

</div>

### `xlinkError` Enum

xlink uses descriptive factory methods for error creation:

```rust
use xlink::core::error::xlinkError;

// Factory methods for common errors:
xlinkError::channel_disconnected(channel_type, reason, location)
xlinkError::encryption_failed(algorithm, reason, location)
xlinkError::group_not_found(group_id, location)
xlinkError::key_derivation_failed(algorithm, reason, location)
xlinkError::resource_exhausted(resource, current, limit, location)
xlinkError::serialization_failed(operation, reason, location)
xlinkError::stream_init_failed(operation, reason, location)
xlinkError::stream_disconnected(stream_id, reason, location)
xlinkError::device_not_found(device_id, location)
xlinkError::invalid_input(field, reason, location)
```

### Error Code Format

Errors use a module-sequence format (XX-YYYY):
- **01xx**: System errors
- **02xx**: Channel errors
- **03xx**: Cryptographic errors
- **04xx**: Group errors
- **05xx**: Device errors
- **06xx**: Stream errors
- **07xx**: Storage errors
- **08xx**: Protocol errors
- **09xx**: Capability errors

### Error Handling Pattern

<table>
<tr>
<td width="50%">

**Pattern Matching**
```rust
match operation() {
    Ok(result) => {
        println!("Success: {:?}", result);
    }
    Err(Error::KeyNotFound) => {
        eprintln!("Key not found");
    }
    Err(Error::EncryptionFailed) => {
        eprintln!("Encryption failed");
    }
    Err(e) => {
        eprintln!("Error: {:?}", e);
    }
}
```

</td>
<td width="50%">

**? Operator**
```rust
fn process_data() -> Result<(), Error> {
    init()?;
    
    let km = KeyManager::new()?;
    let key = km.generate_key(
        Algorithm::AES256GCM
    )?;
    
    let cipher = Cipher::new(
        Algorithm::AES256GCM
    )?;
    
    Ok(())
}
```

</td>
</tr>
</table>

---

## Type Definitions

### Common Types

<table>
<tr>
<td width="50%">

**Key ID**
```rust
pub type KeyId = String;
```

**Algorithm Type**
```rust
pub enum Algorithm { /* ... */ }
```

</td>
<td width="50%">

**Result Type**
```rust
pub type Result<T> = 
    std::result::Result<T, Error>;
```

**Log Level**
```rust
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}
```

</td>
</tr>
</table>

---

## Examples

<div align="center">

### 💡 Common Usage Patterns

</div>

### Example 1: Basic Usage

```rust
use xlink::XLink;
use xlink::core::types::{DeviceCapabilities, DeviceId, MessagePayload, ChannelType};
use std::collections::HashSet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device_id = DeviceId::new();
    let capabilities = DeviceCapabilities {
        device_id,
        device_type: xlink::core::types::DeviceType::Smartphone,
        device_name: "My Device".to_string(),
        supported_channels: HashSet::from([ChannelType::Lan]),
        battery_level: Some(80),
        is_charging: false,
        data_cost_sensitive: false,
    };

    let sdk = XLink::new(capabilities, vec![]).await?;
    sdk.start().await?;

    let payload = MessagePayload::Text("Hello, World!".to_string());
    sdk.send(recipient_id, payload).await?;
    sdk.stop().await;

    println!("✅ Message sent successfully!");
    Ok(())
}
```

### Example 2: Group Messaging

```rust
use xlink::XLink;
use xlink::core::types::{DeviceCapabilities, DeviceId, MessagePayload, ChannelType};
use std::collections::HashSet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device_id = DeviceId::new();
    let capabilities = DeviceCapabilities {
        device_id,
        device_type: xlink::core::types::DeviceType::Smartphone,
        device_name: "My Device".to_string(),
        supported_channels: HashSet::from([ChannelType::Lan]),
        battery_level: Some(80),
        is_charging: false,
        data_cost_sensitive: false,
    };

    let sdk = XLink::new(capabilities, vec![]).await?;
    sdk.start().await?;

    // Create a group with multiple members
    let group_id = sdk.create_group(
        "Team Alpha".to_string(),
        vec![member_1_id, member_2_id, member_3_id]
    ).await?;

    // Broadcast to group
    let payload = MessagePayload::Text("Hello everyone!".to_string());
    sdk.send_to_group(group_id, payload).await?;
    sdk.stop().await;

    println!("✅ Group message sent!");
    Ok(())
}
```

### Example 3: E2E Encrypted Communication

```rust
use xlink::XLink;
use xlink::core::types::{DeviceCapabilities, DeviceId, ChannelType};
use std::collections::HashSet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let device_id = DeviceId::new();
    let capabilities = DeviceCapabilities {
        device_id,
        device_type: xlink::core::types::DeviceType::Smartphone,
        device_name: "My Device".to_string(),
        supported_channels: HashSet::from([ChannelType::Lan]),
        battery_level: Some(80),
        is_charging: false,
        data_cost_sensitive: false,
    };

    let sdk = XLink::new(capabilities, vec![]).await?;
    sdk.start().await?;

    // Register peer's public key for encrypted group communication
    let peer_id: DeviceId = "550e8400-e29b-41d4-a716-446655440000".parse()?;
    sdk.register_device_key(peer_id, peer_public_key)?;

    // Create an encrypted group
    let group_id = sdk.create_group(
        "Secure Group".to_string(),
        vec![peer_id]
    ).await?;

    println!("✅ Secure session established!");
    Ok(())
}
```

---

<div align="center">

**[📖 User Guide](USER_GUIDE.md)** • **[🏗️ Architecture](ARCHITECTURE.md)** • **[🏠 Home](../README.md)**

Made with ❤️ by the Documentation Team

[⬆ Back to Top](#-api-reference)

</div>