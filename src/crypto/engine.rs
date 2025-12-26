use crate::core::error::{Result, XPushError};
use crate::core::types::DeviceId;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use dashmap::DashMap;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::{Arc, Mutex};
use x25519_dalek::{PublicKey, StaticSecret};

// 32字节密钥
type Key = [u8; 32];

/// 会话状态，包含发送和接收链的密钥
#[derive(Serialize, Deserialize)]
struct SessionState {
    // 根密钥，用于派生新的链密钥
    _root_key: Key,
    // 发送链密钥
    send_chain_key: Key,
    // 接收链密钥
    recv_chain_key: Key,
    // 发送计数器
    send_ratchet_counter: u32,
    // 远端验证密钥 (Ed25519)
    #[serde(with = "verifying_key_serde")]
    peer_verifying_key: Option<VerifyingKey>,
}

mod verifying_key_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(
        key: &Option<VerifyingKey>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match key {
            Some(k) => serializer.serialize_some(k.as_bytes()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> std::result::Result<Option<VerifyingKey>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<Vec<u8>> = Option::deserialize(deserializer)?;
        match opt {
            Some(bytes) => {
                let bytes: [u8; 32] = bytes
                    .try_into()
                    .map_err(|_| serde::de::Error::custom("Invalid length"))?;
                VerifyingKey::from_bytes(&bytes)
                    .map(Some)
                    .map_err(|e| serde::de::Error::custom(e.to_string()))
            }
            None => Ok(None),
        }
    }
}

/// 导出的加密引擎状态
#[derive(Serialize, Deserialize)]
pub struct CryptoState {
    pub static_secret: [u8; 32],
    pub signing_key: [u8; 32],
    pub sessions: Vec<(DeviceId, Vec<u8>)>, // 使用 JSON 序列化后的 SessionState
}

impl SessionState {
    fn new(shared_secret: Key, peer_verifying_key: Option<VerifyingKey>) -> Result<Self> {
        // 初始化：使用共享密钥作为根密钥，并派生初始链密钥
        let (root, chain) = kdf_rk(&shared_secret, b"init")?;
        Ok(Self {
            _root_key: root,
            send_chain_key: chain, // 简化：初始双方对称，实际需区分 Initiator/Responder
            recv_chain_key: chain,
            send_ratchet_counter: 0,
            peer_verifying_key,
        })
    }
}

impl Default for CryptoEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// 密钥派生函数 (Root Key KDF)
fn kdf_rk(rk: &Key, info: &[u8]) -> Result<(Key, Key)> {
    let hk = Hkdf::<Sha256>::new(Some(rk), info);
    let mut okm = [0u8; 64];
    hk.expand(&[], &mut okm)
        .map_err(|e| XPushError::CryptoError(format!("HKDF expand failed: {}", e)))?;
    let mut new_rk = [0u8; 32];
    let mut new_ck = [0u8; 32];
    new_rk.copy_from_slice(&okm[0..32]);
    new_ck.copy_from_slice(&okm[32..64]);
    Ok((new_rk, new_ck))
}

/// 链密钥派生函数 (Chain Key KDF) -> (Next Chain Key, Message Key)
fn kdf_ck(ck: &Key) -> Result<(Key, Key)> {
    let hk = Hkdf::<Sha256>::new(Some(ck), b"message_key");
    let mut okm = [0u8; 64];
    hk.expand(&[], &mut okm)
        .map_err(|e| XPushError::CryptoError(format!("HKDF expand failed: {}", e)))?;
    let mut next_ck = [0u8; 32];
    let mut msg_key = [0u8; 32];
    next_ck.copy_from_slice(&okm[0..32]);
    msg_key.copy_from_slice(&okm[32..64]);
    Ok((next_ck, msg_key))
}

pub struct CryptoEngine {
    static_secret: StaticSecret,
    public_key: PublicKey,
    signing_key: SigningKey,
    // DeviceId -> SessionState
    sessions: Arc<DashMap<DeviceId, Mutex<SessionState>>>,
}

impl CryptoEngine {
    pub fn new() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);
        let signing_key = SigningKey::generate(&mut OsRng);
        Self {
            static_secret: secret,
            public_key: public,
            signing_key,
            sessions: Arc::new(DashMap::new()),
        }
    }

    pub fn public_key(&self) -> PublicKey {
        self.public_key
    }

    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// 导出加密引擎状态，用于设备迁移
    pub fn export_state(&self) -> Result<CryptoState> {
        let mut session_data = Vec::new();
        for entry in self.sessions.iter() {
            let device_id = *entry.key();
            let session = entry
                .value()
                .lock()
                .map_err(|_| XPushError::CryptoError("Session mutex poisoned".into()))?;
            let serialized = serde_json::to_vec(&*session).map_err(|e| {
                XPushError::CryptoError(format!("Failed to serialize session: {}", e))
            })?;
            session_data.push((device_id, serialized));
        }

        Ok(CryptoState {
            static_secret: self.static_secret.to_bytes(),
            signing_key: self.signing_key.to_bytes(),
            sessions: session_data,
        })
    }

    /// 从导出的状态导入，用于设备迁移
    pub fn import_state(state: CryptoState) -> Result<Self> {
        let static_secret = StaticSecret::from(state.static_secret);
        let public_key = PublicKey::from(&static_secret);
        let signing_key = SigningKey::from_bytes(&state.signing_key);

        let sessions = Arc::new(DashMap::new());
        for (device_id, serialized) in state.sessions {
            let session: SessionState = serde_json::from_slice(&serialized).map_err(|e| {
                XPushError::CryptoError(format!("Failed to deserialize session: {}", e))
            })?;
            sessions.insert(device_id, Mutex::new(session));
        }

        Ok(Self {
            static_secret,
            public_key,
            signing_key,
            sessions,
        })
    }

    /// 清理所有会话，防止内存泄漏 - use proper entry removal to avoid DashMap fragmentation
    pub fn clear_sessions(&self) {
        // Remove sessions entries one by one to avoid fragmentation
        let session_keys: Vec<_> = self.sessions.iter().map(|entry| *entry.key()).collect();
        for device_id in session_keys {
            self.sessions.remove(&device_id);
        }
    }

    /// 建立会话（模拟 X3DH 的最后一步）
    pub fn establish_session(&self, peer_id: DeviceId, peer_public: PublicKey) -> Result<()> {
        let shared_secret = self.static_secret.diffie_hellman(&peer_public);
        let session = SessionState::new(*shared_secret.as_bytes(), None)?;
        self.sessions.insert(peer_id, Mutex::new(session));
        Ok(())
    }

    /// 建立带身份验证的会话
    pub fn establish_authenticated_session(
        &self,
        peer_id: DeviceId,
        peer_public: PublicKey,
        peer_verifying_key: VerifyingKey,
    ) -> Result<()> {
        let shared_secret = self.static_secret.diffie_hellman(&peer_public);
        let session = SessionState::new(*shared_secret.as_bytes(), Some(peer_verifying_key))?;
        self.sessions.insert(peer_id, Mutex::new(session));
        Ok(())
    }

    /// 对数据进行签名
    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        self.signing_key.sign(data).to_bytes().to_vec()
    }

    /// 验证签名
    pub fn verify(&self, peer_id: &DeviceId, data: &[u8], signature_bytes: &[u8]) -> Result<()> {
        let session_guard = self
            .sessions
            .get(peer_id)
            .ok_or(XPushError::CryptoError("Session not established".into()))?;
        let session = session_guard
            .lock()
            .map_err(|_| XPushError::CryptoError("Session mutex poisoned".into()))?;

        let verifying_key = session
            .peer_verifying_key
            .ok_or(XPushError::CryptoError("No verifying key for peer".into()))?;

        let signature = Signature::from_slice(signature_bytes)
            .map_err(|e| XPushError::CryptoError(format!("Invalid signature format: {}", e)))?;

        verifying_key
            .verify(data, &signature)
            .map_err(|_| XPushError::CryptoError("Signature verification failed".into()))
    }

    /// 加密消息并滚动棘轮 (Forward Secrecy)
    pub fn encrypt(&self, peer_id: &DeviceId, plaintext: &[u8]) -> Result<Vec<u8>> {
        let session_guard = self
            .sessions
            .get(peer_id)
            .ok_or(XPushError::CryptoError("Session not established".into()))?;
        let mut session = session_guard
            .lock()
            .map_err(|_| XPushError::CryptoError("Session mutex poisoned".into()))?;

        // 1. 棘轮步进：生成消息密钥
        let (next_ck, msg_key) = kdf_ck(&session.send_chain_key)?;
        session.send_chain_key = next_ck;
        session.send_ratchet_counter += 1;

        // 2. 加密
        let cipher = ChaCha20Poly1305::new(&msg_key.into());
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| XPushError::CryptoError(e.to_string()))?;

        // 3. 打包: Nonce + Ciphertext
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// 解密消息并滚动接收棘轮
    pub fn decrypt(&self, peer_id: &DeviceId, ciphertext_data: &[u8]) -> Result<Vec<u8>> {
        if ciphertext_data.len() < 12 {
            return Err(XPushError::CryptoError("Data too short".into()));
        }

        let session_guard = self
            .sessions
            .get(peer_id)
            .ok_or(XPushError::CryptoError("Session not established".into()))?;
        let mut session = session_guard
            .lock()
            .map_err(|_| XPushError::CryptoError("Session mutex poisoned".into()))?;

        // 1. 棘轮步进：生成消息密钥
        // 注意：真实实现需要处理乱序消息（保存跳过的 Message Keys），这里简化为必须顺序接收
        let (next_ck, msg_key) = kdf_ck(&session.recv_chain_key)?;
        session.recv_chain_key = next_ck;

        // 2. 解密
        let cipher = ChaCha20Poly1305::new(&msg_key.into());
        let nonce = Nonce::from_slice(&ciphertext_data[..12]);
        let ciphertext = &ciphertext_data[12..];

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| XPushError::CryptoError(e.to_string()))
    }
}
