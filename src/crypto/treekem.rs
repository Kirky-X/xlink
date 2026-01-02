use chacha20poly1305::{
    aead::{Aead, OsRng},
    KeyInit, XChaCha20Poly1305,
};
use dashmap::DashMap;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

use crate::core::error::XLinkError;
use crate::core::types::{DeviceId, GroupId, MessagePayload};

type Key = [u8; 32];

#[derive(Clone, Serialize, Deserialize)]
pub struct TreeKemNode {
    pub node_id: u32,
    pub public_key: Option<Vec<u8>>,
    #[serde(skip)]
    pub private_key: Option<Vec<u8>>,
    pub parent_id: Option<u32>,
    pub children: Vec<u32>,
}

impl std::fmt::Debug for TreeKemNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TreeKemNode")
            .field("node_id", &self.node_id)
            .field("public_key", &self.public_key.as_ref().map(|k| &k[..]))
            .field("private_key", &"<redacted>")
            .field("parent_id", &self.parent_id)
            .field("children", &self.children)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeKemGroup {
    pub group_id: GroupId,
    pub tree: HashMap<u32, TreeKemNode>,
    pub epoch: u64,
    pub group_secret: Key,
    pub member_devices: HashMap<DeviceId, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePath {
    pub updater_id: DeviceId,
    pub path_secrets: Vec<Key>,
    pub path_public_keys: Vec<Vec<u8>>,
    pub epoch: u64,
}

pub struct TreeKemEngine {
    pub groups: Arc<DashMap<GroupId, TreeKemGroup>>,
    pub device_public_keys: Arc<DashMap<DeviceId, Vec<u8>>>,
    pub local_device_id: DeviceId,
    pub local_private_key: StaticSecret,
    pub signing_key: SigningKey,
}

impl TreeKemEngine {
    pub fn new(local_device_id: DeviceId) -> Self {
        let local_private_key = StaticSecret::random_from_rng(OsRng);
        let signing_key = SigningKey::generate(&mut OsRng);

        Self {
            groups: Arc::new(DashMap::new()),
            device_public_keys: Arc::new(DashMap::new()),
            local_device_id,
            local_private_key,
            signing_key,
        }
    }

    pub fn register_device_key(&self, device_id: DeviceId, public_key: X25519PublicKey) {
        self.device_public_keys
            .insert(device_id, public_key.to_bytes().to_vec());
    }

    pub fn get_device_public_key(&self, device_id: DeviceId) -> Result<Vec<u8>, XLinkError> {
        self.device_public_keys
            .get(&device_id)
            .map(|k| k.clone())
            .ok_or_else(|| {
                XLinkError::key_derivation_failed(
                    "X25519",
                    &format!("Device key not found: {}", device_id),
                    file!(),
                )
            })
    }

    pub fn clear_keys(&self) {
        self.groups.clear();
        self.device_public_keys.clear();
    }

    pub fn create_group(
        &self,
        group_id: GroupId,
        member_ids: Vec<DeviceId>,
    ) -> Result<TreeKemGroup, XLinkError> {
        let mut tree = HashMap::new();
        let mut member_devices = HashMap::new();

        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);

        for (index, member_id) in member_ids.iter().enumerate() {
            let node_id = (index + 1) as u32;
            let node = TreeKemNode {
                node_id,
                public_key: self.device_public_keys.get(member_id).map(|k| k.clone()),
                private_key: None,
                parent_id: None,
                children: vec![],
            };
            tree.insert(node_id, node);
            member_devices.insert(*member_id, node_id);

            let parent_id = node_id / 2;
            if parent_id > 0 {
                if let Some(parent) = tree.get_mut(&parent_id) {
                    parent.children.push(node_id);
                }
            }
        }

        let group = TreeKemGroup {
            group_id,
            tree,
            epoch: 0,
            group_secret: secret,
            member_devices,
        };

        self.groups.insert(group_id, group.clone());
        Ok(group)
    }

    pub fn encrypt_group_message(
        &self,
        group_id: GroupId,
        payload: &MessagePayload,
    ) -> Result<MessagePayload, XLinkError> {
        let group = self
            .groups
            .get(&group_id)
            .ok_or_else(|| XLinkError::group_not_found(group_id.to_string(), file!()))?;

        let plaintext = serde_json::to_vec(payload).map_err(Into::<XLinkError>::into)?;

        let mut nonce_bytes = [0u8; 24];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = chacha20poly1305::XNonce::from(nonce_bytes);

        let cipher = XChaCha20Poly1305::new_from_slice(&group.group_secret).map_err(|e| {
            XLinkError::encryption_failed("XChaCha20Poly1305 init", &e.to_string(), file!())
        })?;

        let ciphertext = cipher.encrypt(&nonce, &*plaintext).map_err(|e| {
            XLinkError::encryption_failed("XChaCha20Poly1305 encrypt", &e.to_string(), file!())
        })?;

        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);
        Ok(MessagePayload::Binary(result))
    }

    pub fn decrypt_group_message(
        &self,
        group_id: GroupId,
        payload: &MessagePayload,
    ) -> Result<MessagePayload, XLinkError> {
        match payload {
            MessagePayload::Binary(ciphertext) => {
                if ciphertext.len() < 24 {
                    return Err(XLinkError::invalid_ciphertext(
                        "Ciphertext too short".to_string(),
                        file!(),
                    ));
                }

                let group = self
                    .groups
                    .get(&group_id)
                    .ok_or_else(|| XLinkError::group_not_found(group_id.to_string(), file!()))?;

                let mut nonce_bytes = [0u8; 24];
                nonce_bytes.copy_from_slice(&ciphertext[0..24]);
                let nonce = chacha20poly1305::XNonce::from(nonce_bytes);

                let cipher =
                    XChaCha20Poly1305::new_from_slice(&group.group_secret).map_err(|e| {
                        XLinkError::encryption_failed(
                            "XChaCha20Poly1305 init",
                            &e.to_string(),
                            file!(),
                        )
                    })?;

                let decrypted = cipher.decrypt(&nonce, &ciphertext[24..]).map_err(|e| {
                    XLinkError::encryption_failed(
                        "XChaCha20Poly1305 decrypt",
                        &e.to_string(),
                        file!(),
                    )
                })?;

                let payload: MessagePayload =
                    serde_json::from_slice(&decrypted).map_err(Into::<XLinkError>::into)?;

                Ok(payload)
            }
            _ => Err(XLinkError::invalid_payload_type(
                "Expected binary payload for group message".to_string(),
                &[],
                file!(),
            )),
        }
    }

    pub fn rotate_group_key(&self, group_id: GroupId) -> Result<(), XLinkError> {
        let mut group = self
            .groups
            .get_mut(&group_id)
            .ok_or_else(|| XLinkError::group_not_found(group_id.to_string(), file!()))?;

        let mut new_secret = [0u8; 32];
        OsRng.fill_bytes(&mut new_secret);

        let info = b"xLink_TreeKEM_KeyRotation_v1".to_vec();
        let hkdf = Hkdf::<Sha256>::new(Some(&group.group_secret), &new_secret);
        let mut okm = [0u8; 64];
        hkdf.expand(&info, &mut okm).expect("HKDF key expansion failed");

        group.group_secret.copy_from_slice(&okm[0..32]);
        group.epoch += 1;

        Ok(())
    }

    pub fn update_group_key(
        &self,
        group_id: GroupId,
        device_id: DeviceId,
    ) -> Result<UpdatePath, XLinkError> {
        self.rotate_group_key(group_id)?;
        self.add_member(group_id, device_id)
    }

    pub fn apply_update_path(
        &self,
        group_id: GroupId,
        update_path: &UpdatePath,
    ) -> Result<(), XLinkError> {
        let mut group = self
            .groups
            .get_mut(&group_id)
            .ok_or_else(|| XLinkError::group_not_found(group_id.to_string(), file!()))?;

        for (i, path_secret) in update_path.path_secrets.iter().enumerate() {
            let node_id = (i + 1) as u32;
            if let Some(node) = group.tree.get_mut(&node_id) {
                node.private_key = Some(path_secret.to_vec());
            }
        }

        group.epoch = update_path.epoch;
        Ok(())
    }

    pub fn add_member(
        &self,
        group_id: GroupId,
        device_id: DeviceId,
    ) -> Result<UpdatePath, XLinkError> {
        let mut group = self
            .groups
            .get_mut(&group_id)
            .ok_or_else(|| XLinkError::group_not_found(group_id.to_string(), file!()))?;

        let new_node_id = (group.tree.len() + 1) as u32;
        let private_key = StaticSecret::random_from_rng(OsRng);
        let public_key = X25519PublicKey::from(&private_key);

        let node = TreeKemNode {
            node_id: new_node_id,
            public_key: Some(public_key.to_bytes().to_vec()),
            private_key: Some(private_key.to_bytes().to_vec()),
            parent_id: None,
            children: vec![],
        };

        group.tree.insert(new_node_id, node);
        group.member_devices.insert(device_id, new_node_id);

        let secret = Self::derive_path_secret(&group.group_secret, new_node_id);

        let info = b"xLink_TreeKEM_PathSecret";
        let hkdf = Hkdf::<Sha256>::new(None, &secret);
        let mut okm = [0u8; 64];
        hkdf.expand(info, &mut okm).expect("HKDF path secret expansion failed");

        let path_secret = {
            let mut ps = [0u8; 32];
            ps.copy_from_slice(&okm[0..32]);
            ps
        };

        let mut path_secrets = vec![path_secret];
        let mut current_node = new_node_id;
        while current_node > 0 {
            let parent_id = current_node / 2;
            if parent_id == 0 {
                break;
            }
            let secret = Self::derive_path_secret(path_secrets.last().expect("Path secrets should not be empty"), parent_id);
            let info = b"xLink_TreeKEM_PathSecret";
            let hkdf = Hkdf::<Sha256>::new(None, &secret);
            let mut okm = [0u8; 64];
            hkdf.expand(info, &mut okm).expect("HKDF path secret expansion failed");

            let parent_secret = {
                let mut ps = [0u8; 32];
                ps.copy_from_slice(&okm[0..32]);
                ps
            };
            path_secrets.push(parent_secret);
            current_node = parent_id;
        }

        let mut path_public_keys = vec![];
        let mut current_node = new_node_id;
        while current_node > 0 {
            let parent_id = current_node / 2;
            if parent_id == 0 {
                break;
            }
            if let Some(parent) = group.tree.get(&parent_id) {
                if let Some(pk) = &parent.public_key {
                    path_public_keys.push(pk.clone());
                }
            }
            current_node = parent_id;
        }

        Ok(UpdatePath {
            updater_id: self.local_device_id,
            path_secrets,
            path_public_keys,
            epoch: group.epoch,
        })
    }

    pub fn remove_member(&self, group_id: GroupId, device_id: DeviceId) -> Result<(), XLinkError> {
        let mut group = self
            .groups
            .get_mut(&group_id)
            .ok_or_else(|| XLinkError::group_not_found(group_id.to_string(), file!()))?;

        if let Some(&node_id) = group.member_devices.get(&device_id) {
            if let Some(node) = group.tree.get_mut(&node_id) {
                node.public_key = None;
                node.private_key = None;
            }
            group.member_devices.remove(&device_id);
        }

        Ok(())
    }

    fn derive_path_secret(secret: &Key, node_id: u32) -> Key {
        let mut info = b"xLink_TreeKEM_PathSecret_v1".to_vec();
        info.extend_from_slice(&node_id.to_le_bytes());

        let hkdf = Hkdf::<Sha256>::new(Some(secret), secret);
        let mut okm = [0u8; 32];
        hkdf.expand(&info, &mut okm).expect("HKDF key expansion failed");
        okm
    }

    pub fn sign_message(&self, data: &[u8]) -> Result<Signature, XLinkError> {
        self.signing_key.try_sign(data).map_err(|e| {
            XLinkError::crypto_init_failed(
                format!("Ed25519 signature creation failed: {}", e),
                file!(),
            )
        })
    }

    pub fn verify_signature(
        &self,
        data: &[u8],
        signature: &Signature,
        public_key: &[u8],
    ) -> Result<bool, XLinkError> {
        let pk: [u8; 32] = public_key.try_into().map_err(|_| {
            XLinkError::crypto_init_failed("Invalid Ed25519 public key length", file!())
        })?;
        let verifying_key = VerifyingKey::from_bytes(&pk).map_err(|e| {
            XLinkError::crypto_init_failed(format!("Invalid Ed25519 public key: {}", e), file!())
        })?;
        verifying_key
            .verify(data, signature)
            .map(|_| true)
            .map_err(|e| {
                XLinkError::signature_verification_failed("Ed25519", &e.to_string(), file!())
            })
    }
}
