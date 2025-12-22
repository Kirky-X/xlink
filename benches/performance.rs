use criterion::{black_box, criterion_group, criterion_main, Criterion};
use uuid::Uuid;
use xpush::core::types::{ChannelType, DeviceId};
use xpush::crypto::engine::CryptoEngine;
use xpush::router::predictor::RoutePredictor;

fn crypto_benchmark(c: &mut Criterion) {
    let engine = CryptoEngine::new();
    let device_id = DeviceId(Uuid::new_v4());
    let data = vec![0u8; 1024]; // 1KB data

    // 预先建立会话，否则加密会失败
    let other_public_key = x25519_dalek::PublicKey::from(&x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng));
    engine.establish_session(device_id, other_public_key).unwrap();

    c.bench_function("encrypt_1kb", |b| b.iter(|| {
        engine.encrypt(black_box(&device_id), black_box(&data)).unwrap()
    }));
}

fn predictor_benchmark(c: &mut Criterion) {
    let predictor = RoutePredictor::new();
    let device_id = DeviceId(Uuid::new_v4());
    let channels = vec![ChannelType::Lan, ChannelType::BluetoothLE, ChannelType::Internet];

    // 填充一些历史数据
    for _ in 0..100 {
        predictor.record_result(device_id, ChannelType::Lan, true, Some(10));
    }

    c.bench_function("predict_best_channel", |b| b.iter(|| {
        predictor.predict_best_channel(black_box(device_id), black_box(&channels))
    }));
}

criterion_group!(benches, crypto_benchmark, predictor_benchmark);
criterion_main!(benches);
