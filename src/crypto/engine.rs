use crate::core::error::{Result, XLinkError};
use crate::core::types::DeviceId;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use dashmap::DashMap;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use parking_lot::Mutex;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::Zeroize;

type Key = [u8; 32];

/// 检查密钥是否为弱密钥或全零密钥
fn is_weak_key(key: &[u8]) -> bool {
    // 检查是否全为零
    if key.iter().all(|&b| b == 0) {
        return true;
    }

    // 检查是否全为相同字节（如 0xFF）
    if key.iter().all(|&b| b == key[0]) {
        return true;
    }

    // 检查熵值（简单的熵估计）
    let mut unique_bytes = [false; 256];
    for &byte in key {
        unique_bytes[byte as usize] = true;
    }
    let unique_count = unique_bytes.iter().filter(|&&b| b).count();

    // 如果唯一字节数少于8，认为密钥强度不足
    unique_count < 8
}

/// 安全地派生密钥，包含验证和清理
fn secure_kdf_rk(rk: &Key, info: &[u8]) -> Result<(Key, Key)> {
    // 验证输入密钥
    if is_weak_key(rk) {
        return Err(XLinkError::key_derivation_failed(
            "HKDF-RK",
            "Weak or invalid root key detected",
            file!(),
        ));
    }

    let hk = Hkdf::<Sha256>::new(Some(rk), info);
    let mut okm = [0u8; 64];

    hk.expand(&[], &mut okm)
        .map_err(|e| XLinkError::key_derivation_failed("HKDF-RK", &e.to_string(), file!()))?;

    let mut new_rk = [0u8; 32];
    let mut new_ck = [0u8; 32];
    new_rk.copy_from_slice(&okm[0..32]);
    new_ck.copy_from_slice(&okm[32..64]);

    // 清理临时输出
    okm.zeroize();

    // 验证派生的密钥
    if is_weak_key(&new_rk) || is_weak_key(&new_ck) {
        new_rk.zeroize();
        new_ck.zeroize();
        return Err(XLinkError::key_derivation_failed(
            "HKDF-RK",
            "Derived key is weak, retry key exchange",
            file!(),
        ));
    }

    Ok((new_rk, new_ck))
}

/// 安全地派生消息密钥
fn secure_kdf_ck(ck: &Key) -> Result<(Key, Key)> {
    // 验证输入密钥
    if is_weak_key(ck) {
        return Err(XLinkError::key_derivation_failed(
            "HKDF-CK",
            "Weak or invalid chain key detected",
            file!(),
        ));
    }

    let hk = Hkdf::<Sha256>::new(Some(ck), b"message_key");
    let mut okm = [0u8; 64];

    hk.expand(&[], &mut okm)
        .map_err(|e| XLinkError::key_derivation_failed("HKDF-CK", &e.to_string(), file!()))?;

    let mut next_ck = [0u8; 32];
    let mut msg_key = [0u8; 32];
    next_ck.copy_from_slice(&okm[0..32]);
    msg_key.copy_from_slice(&okm[32..64]);

    // 清理临时输出
    okm.zeroize();

    // 验证派生的密钥
    if is_weak_key(&next_ck) || is_weak_key(&msg_key) {
        next_ck.zeroize();
        msg_key.zeroize();
        return Err(XLinkError::key_derivation_failed(
            "HKDF-CK",
            "Derived message key is weak, retry key exchange",
            file!(),
        ));
    }

    Ok((next_ck, msg_key))
}

#[derive(Serialize, Deserialize)]
struct SessionState {
    _root_key: Key,
    send_chain_key: Key,
    recv_chain_key: Key,
    send_ratchet_counter: u32,
    #[serde(with = "verifying_key_serde")]
    peer_verifying_key: Option<VerifyingKey>,
    /// 会话创建时间戳（秒）
    created_at: u64,
    /// 会话过期时间（秒），默认 24 小时
    expires_at: u64,
}

// 实现 Drop trait 以确保会话结束时安全清理密钥
impl Drop for SessionState {
    fn drop(&mut self) {
        // 安全地清理所有密钥材料
        self._root_key.zeroize();
        self.send_chain_key.zeroize();
        self.recv_chain_key.zeroize();
    }
}

impl SessionState {
    const SESSION_TTL_SECONDS: u64 = 24 * 60 * 60; // 24小时

    fn new(shared_secret: Key, peer_verifying_key: Option<VerifyingKey>) -> Result<Self> {
        // 验证共享密钥
        if is_weak_key(&shared_secret) {
            return Err(XLinkError::key_derivation_failed(
                "X25519",
                "Weak shared secret detected, possible key exchange failure",
                file!(),
            ));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| XLinkError::timeout("System time error", 0, file!()))?
            .as_secs();

        let (root, chain) = secure_kdf_rk(&shared_secret, b"init")?;

        // 清理共享密钥副本
        let mut shared_secret_copy = shared_secret;
        shared_secret_copy.zeroize();

        Ok(Self {
            _root_key: root,
            send_chain_key: chain,
            recv_chain_key: chain,
            send_ratchet_counter: 0,
            peer_verifying_key,
            created_at: now,
            expires_at: now + Self::SESSION_TTL_SECONDS,
        })
    }

    /// 检查会话是否已过期
    fn is_expired(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now > self.expires_at
    }
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

#[derive(Serialize, Deserialize)]
pub struct CryptoState {
    pub static_secret: [u8; 32],
    pub signing_key: [u8; 32],
    pub sessions: Vec<(DeviceId, Vec<u8>)>,
}

pub struct CryptoEngine {
    static_secret: StaticSecret,
    public_key: PublicKey,
    signing_key: SigningKey,
    /// 使用 Mutex 替代 RwLock 避免嵌套锁死锁风险
    /// 访问模式：先通过 DashMap 获取条目，再获取 Mutex 锁
    sessions: Arc<DashMap<DeviceId, Mutex<SessionState>>>,
}

impl Default for CryptoEngine {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn export_state(&self) -> Result<CryptoState> {
        let mut session_data = Vec::new();
        for entry in self.sessions.iter() {
            let device_id = *entry.key();
            // 使用 Mutex 锁代替 RwLock read()，避免死锁风险
            let session = entry.value().lock();
            let serialized = serde_json::to_vec(&*session).map_err(Into::<XLinkError>::into)?;
            session_data.push((device_id, serialized));
        }

        Ok(CryptoState {
            static_secret: self.static_secret.to_bytes(),
            signing_key: self.signing_key.to_bytes(),
            sessions: session_data,
        })
    }

    pub fn import_state(state: CryptoState) -> Result<Self> {
        let static_secret = StaticSecret::from(state.static_secret);
        let public_key = PublicKey::from(&static_secret);
        let signing_key = SigningKey::from_bytes(&state.signing_key);

        let sessions = Arc::new(DashMap::new());
        for (device_id, serialized) in state.sessions {
            let session: SessionState =
                serde_json::from_slice(&serialized).map_err(Into::<XLinkError>::into)?;
            // 使用 Mutex 替代 RwLock
            sessions.insert(device_id, Mutex::new(session));
        }

        Ok(Self {
            static_secret,
            public_key,
            signing_key,
            sessions,
        })
    }

    pub fn clear_sessions(&self) {
        let session_keys: Vec<_> = self.sessions.iter().map(|entry| *entry.key()).collect();
        for device_id in session_keys {
            self.sessions.remove(&device_id);
        }
    }

    pub fn establish_session(&self, peer_id: DeviceId, peer_public: PublicKey) -> Result<()> {
        let shared_secret = self.static_secret.diffie_hellman(&peer_public);
        let session = SessionState::new(*shared_secret.as_bytes(), None)?;
        // 使用 Mutex 替代 RwLock
        self.sessions.insert(peer_id, Mutex::new(session));
        Ok(())
    }

    pub fn establish_authenticated_session(
        &self,
        peer_id: DeviceId,
        peer_public: PublicKey,
        peer_verifying_key: VerifyingKey,
    ) -> Result<()> {
        let shared_secret = self.static_secret.diffie_hellman(&peer_public);
        let session = SessionState::new(*shared_secret.as_bytes(), Some(peer_verifying_key))?;
        // 使用 Mutex 替代 RwLock
        self.sessions.insert(peer_id, Mutex::new(session));
        Ok(())
    }

    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        self.signing_key.sign(data).to_bytes().to_vec()
    }

    pub fn verify(&self, peer_id: &DeviceId, data: &[u8], signature_bytes: &[u8]) -> Result<()> {
        let session_guard = self
            .sessions
            .get(peer_id)
            .ok_or_else(|| XLinkError::device_not_found(peer_id.to_string(), file!()))?;
        // 使用 Mutex 锁代替 RwLock read()，避免死锁风险
        let session = session_guard.lock();

        // 检查会话是否已过期
        if session.is_expired() {
            drop(session);
            drop(session_guard);
            self.sessions.remove(peer_id);
            return Err(XLinkError::timeout(
                format!("Session expired for device {}", peer_id),
                0,
                file!(),
            ));
        }

        let verifying_key = session.peer_verifying_key.ok_or_else(|| {
            XLinkError::invalid_input("verifying_key", "No verifying key for peer", file!())
        })?;

        let signature = Signature::from_slice(signature_bytes).map_err(|e| {
            XLinkError::signature_verification_failed("Ed25519", &e.to_string(), file!())
        })?;

        verifying_key.verify(data, &signature).map_err(|e| {
            XLinkError::signature_verification_failed("Ed25519", &e.to_string(), file!())
        })
    }

    pub fn encrypt(&self, peer_id: &DeviceId, plaintext: &[u8]) -> Result<Vec<u8>> {
        let session_guard = self
            .sessions
            .get(peer_id)
            .ok_or_else(|| XLinkError::device_not_found(peer_id.to_string(), file!()))?;
        // 使用 Mutex 锁代替 RwLock write()，避免死锁风险
        let mut session = session_guard.lock();

        // 检查会话是否已过期
        if session.is_expired() {
            drop(session);
            drop(session_guard);
            self.sessions.remove(peer_id);
            return Err(XLinkError::timeout(
                format!("Session expired for device {}", peer_id),
                0,
                file!(),
            ));
        }

        let (next_ck, msg_key) = secure_kdf_ck(&session.send_chain_key)?;
        session.send_chain_key = next_ck;
        session.send_ratchet_counter += 1;

        let cipher = ChaCha20Poly1305::new((&msg_key).into());
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext).map_err(|e| {
            XLinkError::encryption_failed("ChaCha20Poly1305", &e.to_string(), file!())
        })?;

        // 安全清理消息密钥
        let mut msg_key_copy = msg_key;
        msg_key_copy.zeroize();

        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, peer_id: &DeviceId, ciphertext_data: &[u8]) -> Result<Vec<u8>> {
        if ciphertext_data.len() < 12 {
            return Err(XLinkError::invalid_ciphertext(
                "Ciphertext too short (minimum 12 bytes for nonce)".to_string(),
                file!(),
            ));
        }

        let session_guard = self
            .sessions
            .get(peer_id)
            .ok_or_else(|| XLinkError::device_not_found(peer_id.to_string(), file!()))?;
        // 使用 Mutex 锁代替 RwLock write()，避免死锁风险
        let mut session = session_guard.lock();

        // 检查会话是否已过期
        if session.is_expired() {
            drop(session);
            drop(session_guard);
            self.sessions.remove(peer_id);
            return Err(XLinkError::timeout(
                format!("Session expired for device {}", peer_id),
                0,
                file!(),
            ));
        }

        let (next_ck, msg_key) = secure_kdf_ck(&session.recv_chain_key)?;
        session.recv_chain_key = next_ck;

        let nonce = Nonce::from_slice(&ciphertext_data[..12]);
        let ciphertext = &ciphertext_data[12..];

        let cipher = ChaCha20Poly1305::new((&msg_key).into());

        let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|e| {
            XLinkError::encryption_failed("ChaCha20Poly1305", &e.to_string(), file!())
        })?;

        // 安全清理消息密钥
        let mut msg_key_copy = msg_key;
        msg_key_copy.zeroize();

        Ok(plaintext)
    }
}

pub type PublicKeyAlias = PublicKey;
