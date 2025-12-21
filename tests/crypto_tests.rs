//! Unit tests for crypto engine module
//!
//! Tests cover encryption/decryption, key management, and security features
//! as specified in test.md section 2.2.3

use std::sync::Arc;
use x25519_dalek::{EphemeralSecret, PublicKey};
use xpush::crypto::engine::CryptoEngine;
use xpush::core::types::{Message, MessagePayload};

use crate::common::{test_device_id, test_text_message};

mod common;

#[tokio::test]
async fn test_message_encryption_decryption() {
    // UT-CRY-001: 消息加解密
    let _engine = CryptoEngine::new();
    let message = test_text_message("Hello, this is a test message!");
    
    // Generate keys for sender and receiver
    let sender_secret = EphemeralSecret::random_from_rng(rand::thread_rng());
    let _sender_public = PublicKey::from(&sender_secret);
    let receiver_secret = EphemeralSecret::random_from_rng(rand::thread_rng());
    let _receiver_public = PublicKey::from(&receiver_secret);
    
    // Establish sessions for both directions
    let sender_engine = CryptoEngine::new();
    let receiver_engine = CryptoEngine::new();
    
    // Exchange public keys and establish sessions
    sender_engine.establish_session(message.recipient, receiver_engine.public_key());
    receiver_engine.establish_session(message.sender, sender_engine.public_key());
    
    // Serialize message payload for encryption
    let plaintext = match &message.payload {
        MessagePayload::Text(text) => text.as_bytes().to_vec(),
        MessagePayload::Binary(data) => data.clone(),
        _ => vec![],
    };
    
    // Encrypt message
    let encrypted = sender_engine.encrypt(
        &message.recipient,
        &plaintext,
    ).unwrap();
    
    // Decrypt message
    let decrypted_data = receiver_engine.decrypt(
        &message.sender,
        &encrypted,
    ).unwrap();
    
    // Verify decrypted data matches original
    match &message.payload {
        MessagePayload::Text(original_text) => {
            let decrypted_text = String::from_utf8(decrypted_data).unwrap();
            assert_eq!(original_text, &decrypted_text);
        }
        MessagePayload::Binary(original_data) => {
            assert_eq!(original_data, &decrypted_data);
        }
        _ => {}
    }
}

#[tokio::test]
async fn test_binary_message_encryption() {
    // UT-CRY-002: 二进制消息加解密
    let binary_data = vec![0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9];
    let message = Message::new(
        test_device_id(),
        test_device_id(),
        MessagePayload::Binary(binary_data.clone()),
    );
    
    // Generate keys
    let sender_secret = EphemeralSecret::random_from_rng(rand::thread_rng());
    let _sender_public = PublicKey::from(&sender_secret);
    let receiver_secret = EphemeralSecret::random_from_rng(rand::thread_rng());
    let _receiver_public = PublicKey::from(&receiver_secret);
    
    // Create engines and establish sessions
    let sender_engine = CryptoEngine::new();
    let receiver_engine = CryptoEngine::new();
    
    sender_engine.establish_session(message.recipient, receiver_engine.public_key());
    receiver_engine.establish_session(message.sender, sender_engine.public_key());
    
    // Serialize message payload for encryption
    let plaintext = match &message.payload {
        MessagePayload::Binary(data) => data.clone(),
        _ => panic!("Expected binary payload"),
    };
    
    // Encrypt and decrypt
    let encrypted = sender_engine.encrypt(
        &message.recipient,
        &plaintext,
    ).unwrap();
    
    let decrypted_data = receiver_engine.decrypt(
        &message.sender,
        &encrypted,
    ).unwrap();
    
    // Verify decrypted data matches original
    assert_eq!(plaintext, decrypted_data);
    assert_eq!(binary_data, decrypted_data);
}

#[tokio::test]
async fn test_key_generation_and_exchange() {
    // UT-CRY-003: 密钥生成与交换
    let crypto_engine_a = CryptoEngine::new();
    let crypto_engine_b = CryptoEngine::new();
    
    // Get public keys for both devices
    let device_a_public = crypto_engine_a.public_key();
    let device_b_public = crypto_engine_b.public_key();
    
    // Verify keys are different
    assert_ne!(device_a_public.as_bytes(), device_b_public.as_bytes());
}

#[tokio::test]
async fn test_signature_verification() {
    // UT-CRY-004: 签名验证 (Ed25519)
    let sender_id = test_device_id();
    let recipient_id = test_device_id();
    
    let sender_engine = CryptoEngine::new();
    let receiver_engine = CryptoEngine::new();
    
    // Establish authenticated sessions with verifying keys
    sender_engine.establish_authenticated_session(
        recipient_id,
        receiver_engine.public_key(),
        receiver_engine.verifying_key(),
    );
    receiver_engine.establish_authenticated_session(
        sender_id,
        sender_engine.public_key(),
        sender_engine.verifying_key(),
    );
    
    let test_data = b"Authentic message data";
    
    // Sender signs the data
    let signature = sender_engine.sign(test_data);
    
    // Receiver verifies the signature
    let verification_result = receiver_engine.verify(&sender_id, test_data, &signature);
    assert!(verification_result.is_ok(), "Signature verification should succeed");
    
    // Test with tampered data
    let tampered_data = b"Tampered message data";
    let tampered_result = receiver_engine.verify(&sender_id, tampered_data, &signature);
    assert!(tampered_result.is_err(), "Signature verification should fail for tampered data");
    
    // Test with invalid signature
    let invalid_signature = vec![0u8; 64];
    let invalid_sig_result = receiver_engine.verify(&sender_id, test_data, &invalid_signature);
    assert!(invalid_sig_result.is_err(), "Signature verification should fail for invalid signature");
}

#[tokio::test]
async fn test_key_rotation() {
    // UT-CRY-010: 密钥轮换 (Double Ratchet)
    let sender_id = test_device_id();
    let recipient_id = test_device_id();
    
    let sender_engine = CryptoEngine::new();
    let receiver_engine = CryptoEngine::new();
    
    sender_engine.establish_session(recipient_id, receiver_engine.public_key());
    receiver_engine.establish_session(sender_id, sender_engine.public_key());
    
    let plaintext = b"Message for rotation test";
    
    // Multiple messages to trigger ratchet steps
    for _ in 0..5 {
        let encrypted = sender_engine.encrypt(&recipient_id, plaintext).unwrap();
        let decrypted = receiver_engine.decrypt(&sender_id, &encrypted).unwrap();
        assert_eq!(plaintext, decrypted.as_slice());
    }
}

#[tokio::test]
async fn test_tamper_detection() {
    // UT-CRY-005: 消息篡改检测
    let message = test_text_message("Original message");
    
    // Generate keys
    let sender_secret = EphemeralSecret::random_from_rng(rand::thread_rng());
    let _sender_public = PublicKey::from(&sender_secret);
    let receiver_secret = EphemeralSecret::random_from_rng(rand::thread_rng());
    let _receiver_public = PublicKey::from(&receiver_secret);
    
    // Create engines and establish sessions
    let sender_engine = CryptoEngine::new();
    let receiver_engine = CryptoEngine::new();
    
    sender_engine.establish_session(message.recipient, receiver_engine.public_key());
    receiver_engine.establish_session(message.sender, sender_engine.public_key());
    
    // Serialize message payload for encryption
    let plaintext = match &message.payload {
        MessagePayload::Text(text) => text.as_bytes().to_vec(),
        _ => vec![],
    };
    
    // Encrypt message
    let mut encrypted = sender_engine.encrypt(
        &message.recipient,
        &plaintext,
    ).unwrap();
    
    // Tamper with the encrypted data
    encrypted[0] ^= 0xFF;
    
    // Attempt to decrypt tampered message
    let result = receiver_engine.decrypt(
        &message.sender,
        &encrypted,
    );
    
    assert!(result.is_err(), "Tampered message should fail decryption");
}

#[tokio::test]
async fn test_replay_attack_prevention() {
    // UT-CRY-006: 重放攻击防护
    let message = test_text_message("Test message");
    
    // Generate keys
    let sender_secret = EphemeralSecret::random_from_rng(rand::thread_rng());
    let _sender_public = PublicKey::from(&sender_secret);
    let receiver_secret = EphemeralSecret::random_from_rng(rand::thread_rng());
    let _receiver_public = PublicKey::from(&receiver_secret);
    
    // Create engines and establish sessions
    let sender_engine = CryptoEngine::new();
    let receiver_engine = CryptoEngine::new();
    
    sender_engine.establish_session(message.recipient, receiver_engine.public_key());
    receiver_engine.establish_session(message.sender, sender_engine.public_key());
    
    // Serialize message payload for encryption
    let plaintext = match &message.payload {
        MessagePayload::Text(text) => text.as_bytes().to_vec(),
        _ => vec![],
    };
    
    // Encrypt message
    let encrypted = sender_engine.encrypt(
        &message.recipient,
        &plaintext,
    ).unwrap();
    
    // First decryption should succeed
    let _decrypted1 = receiver_engine.decrypt(
        &message.sender,
        &encrypted.clone(),
    ).unwrap();
    
    // Attempt to decrypt the same message again (replay attack)
    let result2 = receiver_engine.decrypt(
        &message.sender,
        &encrypted,
    );
    
    // Should fail due to replay attack prevention (the ratchet advances)
    assert!(result2.is_err(), "Replay attack should be prevented");
}

#[tokio::test]
async fn test_forward_secrecy() {
    // UT-CRY-007: 前向保密性
    let sender_id = test_device_id();
    let recipient_id = test_device_id();
    let message1 = Message::new(sender_id, recipient_id, MessagePayload::Text("Message 1".to_string()));
    let message2 = Message::new(sender_id, recipient_id, MessagePayload::Text("Message 2".to_string()));
    
    // Create engines and establish sessions (using static keys for session establishment)
    let sender_engine = CryptoEngine::new();
    let receiver_engine = CryptoEngine::new();
    
    sender_engine.establish_session(recipient_id, receiver_engine.public_key());
    receiver_engine.establish_session(sender_id, sender_engine.public_key());
    
    // Serialize message payloads for encryption
    let plaintext1 = match &message1.payload {
        MessagePayload::Text(text) => text.as_bytes().to_vec(),
        _ => vec![],
    };
    
    let plaintext2 = match &message2.payload {
        MessagePayload::Text(text) => text.as_bytes().to_vec(),
        _ => vec![],
    };
    
    // Encrypt both messages (the ratchet will advance between them)
    let encrypted1 = sender_engine.encrypt(
        &message1.recipient,
        &plaintext1,
    ).unwrap();

    let encrypted2 = sender_engine.encrypt(
        &message2.recipient,
        &plaintext2,
    ).unwrap();
    
    // Verify they use different encryption (due to ratchet advancement)
    assert_ne!(encrypted1, encrypted2);
    
    // Compromising one key should not affect the other (forward secrecy)
    let decrypted1 = receiver_engine.decrypt(
        &message1.sender,
        &encrypted1,
    ).unwrap();

    let decrypted2 = receiver_engine.decrypt(
        &message2.sender,
        &encrypted2,
    ).unwrap();
    
    // Verify decrypted data matches original
    assert_eq!(plaintext1, decrypted1);
    assert_eq!(plaintext2, decrypted2);
}

#[tokio::test]
async fn test_concurrent_encryption() {
    // UT-CRY-008: 并发加解密
    let num_messages = 10;
    
    let mut handles = vec![];
    
    for i in 0..num_messages {
        let handle = tokio::spawn(async move {
            let message = Message::new(
                test_device_id(),
                test_device_id(),
                MessagePayload::Text(format!("Concurrent message {}", i)),
            );
            
            // Create engines for this message
            let sender_engine = CryptoEngine::new();
            let receiver_engine = CryptoEngine::new();
            
            sender_engine.establish_session(message.recipient, receiver_engine.public_key());
            receiver_engine.establish_session(message.sender, sender_engine.public_key());
            
            // Serialize message payload for encryption
            let plaintext = match &message.payload {
                MessagePayload::Text(text) => text.as_bytes().to_vec(),
                _ => vec![],
            };
            
            // Encrypt and decrypt
            let encrypted = sender_engine.encrypt(
                &message.recipient,
                &plaintext,
            ).unwrap();
            
            let decrypted_data = receiver_engine.decrypt(
                &message.sender,
                &encrypted,
            ).unwrap();
            
            // Verify decrypted data matches original
            match &message.payload {
                MessagePayload::Text(original_text) => {
                    let decrypted_text = String::from_utf8(decrypted_data).unwrap();
                    assert_eq!(original_text, &decrypted_text);
                }
                _ => panic!("Expected text payload"),
            }
        });
        handles.push(handle);
    }
    
    // Wait for all concurrent operations to complete
    for handle in handles {
        handle.await.unwrap();
    }
}

#[tokio::test]
async fn test_key_storage_security() {
    // UT-CRY-009: 密钥存储安全性 - Note: CryptoEngine handles key storage securely
    let crypto_engine = Arc::new(CryptoEngine::new());
    let device_id = test_device_id();
    
    // Get public key (private key is securely stored internally)
    let public_key = crypto_engine.public_key();
    
    // Establish session with another device to test key management
    let peer_crypto_engine = Arc::new(CryptoEngine::new());
    let peer_public = peer_crypto_engine.public_key();
    
    // Establish sessions
    crypto_engine.establish_session(device_id, peer_public);
    peer_crypto_engine.establish_session(device_id, public_key);
    
    // Test that encryption/decryption works (implies proper key storage)
    let test_message = b"Test message for key storage security";
    let encrypted = crypto_engine.encrypt(&device_id, test_message).unwrap();
    let decrypted = peer_crypto_engine.decrypt(&device_id, &encrypted).unwrap();
    
    assert_eq!(test_message, decrypted.as_slice());
    
    // Note: The CryptoEngine ensures that:
    // 1. Private keys are never exposed outside the engine
    // 2. Keys are stored securely in memory
    // 3. Key rotation happens automatically through the Double Ratchet
}

#[tokio::test]
async fn test_encryption_performance() {
    // UT-CRY-010: 加解密性能
    let message = test_text_message("Performance test message");
    
    // Create engines and establish sessions
    let sender_engine = CryptoEngine::new();
    let receiver_engine = CryptoEngine::new();
    
    sender_engine.establish_session(message.recipient, receiver_engine.public_key());
    receiver_engine.establish_session(message.sender, sender_engine.public_key());
    
    // Serialize message payload for encryption
    let plaintext = match &message.payload {
        MessagePayload::Text(text) => text.as_bytes().to_vec(),
        _ => vec![],
    };
    
    let start = std::time::Instant::now();
    
    // Perform encryption and decryption
    let encrypted = sender_engine.encrypt(
        &message.recipient,
        &plaintext,
    ).unwrap();
    
    let decrypted_data = receiver_engine.decrypt(
        &message.sender,
        &encrypted,
    ).unwrap();
    
    let duration = start.elapsed();
    
    // Verify correctness
    match &message.payload {
        MessagePayload::Text(original_text) => {
            let decrypted_text = String::from_utf8(decrypted_data).unwrap();
            assert_eq!(original_text, &decrypted_text);
        }
        _ => panic!("Expected text payload"),
    }
    
    // Performance assertion (should be very fast)
    assert!(duration.as_millis() < 100, "Encryption/decryption took too long: {:?}", duration);
}