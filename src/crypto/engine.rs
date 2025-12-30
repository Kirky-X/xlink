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

type Key = [u8; 32];

#[derive(Serialize, Deserialize)]
struct SessionState {
    _root_key: Key,
    send_chain_key: Key,
    recv_chain_key: Key,
    send_ratchet_counter: u32,
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

#[derive(Serialize, Deserialize)]
pub struct CryptoState {
    pub static_secret: [u8; 32],
    pub signing_key: [u8; 32],
    pub sessions: Vec<(DeviceId, Vec<u8>)>,
}

impl SessionState {
    fn new(shared_secret: Key, peer_verifying_key: Option<VerifyingKey>) -> Result<Self> {
        let (root, chain) = kdf_rk(&shared_secret, b"init")?;
        Ok(Self {
            _root_key: root,
            send_chain_key: chain,
            recv_chain_key: chain,
            send_ratchet_counter: 0,
            peer_verifying_key,
        })
    }
}

fn kdf_rk(rk: &Key, info: &[u8]) -> Result<(Key, Key)> {
    let hk = Hkdf::<Sha256>::new(Some(rk), info);
    let mut okm = [0u8; 64];
    hk.expand(&[], &mut okm)
        .map_err(|e| XPushError::key_derivation_failed("HKDF-RK", &e.to_string(), file!()))?;
    let mut new_rk = [0u8; 32];
    let mut new_ck = [0u8; 32];
    new_rk.copy_from_slice(&okm[0..32]);
    new_ck.copy_from_slice(&okm[32..64]);
    Ok((new_rk, new_ck))
}

fn kdf_ck(ck: &Key) -> Result<(Key, Key)> {
    let hk = Hkdf::<Sha256>::new(Some(ck), b"message_key");
    let mut okm = [0u8; 64];
    hk.expand(&[], &mut okm)
        .map_err(|e| XPushError::key_derivation_failed("HKDF-CK", &e.to_string(), file!()))?;
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
            let session = entry
                .value()
                .lock()
                .map_err(|_| XPushError::resource_exhausted("Session mutex poisoned".to_string(), 0, 0, file!()))?;
            let serialized = serde_json::to_vec(&*session)
                .map_err(Into::<XPushError>::into)?;
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
            let session: SessionState = serde_json::from_slice(&serialized)
                .map_err(Into::<XPushError>::into)?;
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
            .ok_or_else(|| XPushError::device_not_found(peer_id.to_string(), file!()))?;
        let session = session_guard
            .lock()
            .map_err(|_| XPushError::resource_exhausted("Session mutex poisoned".to_string(), 0, 0, file!()))?;

        let verifying_key = session
            .peer_verifying_key
            .ok_or_else(|| XPushError::invalid_input("verifying_key", "No verifying key for peer", file!()))?;

        let signature = Signature::from_slice(signature_bytes)
            .map_err(|e| XPushError::signature_verification_failed("Ed25519", &e.to_string(), file!()))?;

        verifying_key
            .verify(data, &signature)
            .map_err(|e| XPushError::signature_verification_failed("Ed25519", &e.to_string(), file!()))
    }

    pub fn encrypt(&self, peer_id: &DeviceId, plaintext: &[u8]) -> Result<Vec<u8>> {
        let session_guard = self
            .sessions
            .get(peer_id)
            .ok_or_else(|| XPushError::device_not_found(peer_id.to_string(), file!()))?;
        let mut session = session_guard
            .lock()
            .map_err(|_| XPushError::resource_exhausted("Session mutex poisoned".to_string(), 0, 0, file!()))?;

        let (next_ck, msg_key) = kdf_ck(&session.send_chain_key)?;
        session.send_chain_key = next_ck;
        session.send_ratchet_counter += 1;

        let cipher = ChaCha20Poly1305::new(&msg_key.into());
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| XPushError::encryption_failed("ChaCha20Poly1305", &e.to_string(), file!()))?;

        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, peer_id: &DeviceId, ciphertext_data: &[u8]) -> Result<Vec<u8>> {
        if ciphertext_data.len() < 12 {
            return Err(XPushError::invalid_ciphertext(
                "Ciphertext too short (minimum 12 bytes for nonce)".to_string(),
                file!(),
            ));
        }

        let session_guard = self
            .sessions
            .get(peer_id)
            .ok_or_else(|| XPushError::device_not_found(peer_id.to_string(), file!()))?;
        let mut session = session_guard
            .lock()
            .map_err(|_| XPushError::resource_exhausted("Session mutex poisoned".to_string(), 0, 0, file!()))?;

        let (next_ck, msg_key) = kdf_ck(&session.recv_chain_key)?;
        session.recv_chain_key = next_ck;

        let cipher = ChaCha20Poly1305::new(&msg_key.into());
        let nonce = Nonce::from_slice(&ciphertext_data[..12]);
        let ciphertext = &ciphertext_data[12..];

        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| XPushError::encryption_failed("ChaCha20Poly1305", &e.to_string(), file!()))
    }
}

pub type PublicKeyAlias = PublicKey;
