//! Benchmarks for ALEC encoding/decoding performance

use alec::{Classifier, Context, Decoder, Encoder, RawData};
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

fn generate_test_data(count: usize) -> Vec<RawData> {
    (0..count)
        .map(|i| {
            let value = 20.0 + (i as f64 % 10.0) * 0.1;
            RawData::new(value, i as u64)
        })
        .collect()
}

fn bench_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoding");

    // Setup
    let data = generate_test_data(1000);
    let classifier = Classifier::default();
    let mut context = Context::new();

    // Warm up context
    for d in data.iter().take(100) {
        context.observe(d);
    }

    group.throughput(Throughput::Elements(1000));

    group.bench_function("encode_1000_messages", |b| {
        b.iter(|| {
            let mut encoder = Encoder::new();
            for d in &data {
                let classification = classifier.classify(d, &context);
                let msg = encoder.encode(d, &classification, &context);
                black_box(msg);
            }
        })
    });

    group.finish();
}

fn bench_decoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("decoding");

    // Setup - encode messages first
    let data = generate_test_data(1000);
    let classifier = Classifier::default();
    let mut context = Context::new();

    // Warm up context
    for d in data.iter().take(100) {
        context.observe(d);
    }

    let mut encoder = Encoder::new();
    let messages: Vec<_> = data
        .iter()
        .map(|d| {
            let classification = classifier.classify(d, &context);
            encoder.encode(d, &classification, &context)
        })
        .collect();

    group.throughput(Throughput::Elements(1000));

    group.bench_function("decode_1000_messages", |b| {
        b.iter(|| {
            let mut decoder = Decoder::new();
            for msg in &messages {
                let decoded = decoder.decode(msg, &context);
                black_box(decoded);
            }
        })
    });

    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip");

    let data = generate_test_data(1000);
    let classifier = Classifier::default();
    let mut context = Context::new();

    // Warm up
    for d in data.iter().take(100) {
        context.observe(d);
    }

    group.throughput(Throughput::Elements(1000));

    group.bench_function("encode_decode_1000", |b| {
        b.iter(|| {
            let mut encoder = Encoder::new();
            let mut decoder = Decoder::new();

            for d in &data {
                let classification = classifier.classify(d, &context);
                let msg = encoder.encode(d, &classification, &context);
                let decoded = decoder.decode(&msg, &context);
                black_box(decoded);
            }
        })
    });

    group.finish();
}

fn bench_classification(c: &mut Criterion) {
    let mut group = c.benchmark_group("classification");

    let data = generate_test_data(1000);
    let classifier = Classifier::default();
    let mut context = Context::new();

    // Warm up
    for d in data.iter().take(100) {
        context.observe(d);
    }

    group.throughput(Throughput::Elements(1000));

    group.bench_function("classify_1000", |b| {
        b.iter(|| {
            for d in &data {
                let classification = classifier.classify(d, &context);
                black_box(classification);
            }
        })
    });

    group.finish();
}

fn bench_context_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("context");

    group.bench_function("observe_1000", |b| {
        let data = generate_test_data(1000);
        b.iter(|| {
            let mut context = Context::new();
            for d in &data {
                context.observe(d);
            }
            black_box(context);
        })
    });

    group.bench_function("predict_1000", |b| {
        let data = generate_test_data(100);
        let mut context = Context::new();
        for d in &data {
            context.observe(d);
        }

        b.iter(|| {
            for _ in 0..1000 {
                let pred = context.predict(0);
                black_box(pred);
            }
        })
    });

    group.bench_function("hash_context", |b| {
        let data = generate_test_data(100);
        let mut context = Context::new();
        for d in &data {
            context.observe(d);
        }
        // Add some patterns
        for i in 0..100 {
            context
                .register_pattern(alec::context::Pattern::new(vec![i as u8; 10]))
                .ok();
        }

        b.iter(|| {
            let hash = context.hash();
            black_box(hash);
        })
    });

    group.finish();
}

fn bench_compression_ratio(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    // Test compression ratio with different data patterns
    let patterns = vec![
        ("constant", vec![20.0f64; 100]),
        ("linear", (0..100).map(|i| 20.0 + i as f64 * 0.1).collect()),
        (
            "noisy",
            (0..100)
                .map(|i| 20.0 + ((i * 7) % 10) as f64 * 0.1)
                .collect(),
        ),
        (
            "random",
            (0..100)
                .map(|i| ((i * 12345) % 1000) as f64 / 10.0)
                .collect(),
        ),
    ];

    for (name, values) in patterns {
        group.bench_function(format!("ratio_{}", name), |b| {
            b.iter(|| {
                let mut encoder = Encoder::new();
                let classifier = Classifier::default();
                let mut context = Context::new();

                let mut raw_total = 0usize;
                let mut encoded_total = 0usize;

                for (i, &value) in values.iter().enumerate() {
                    let data = RawData::new(value, i as u64);
                    raw_total += data.raw_size();

                    let classification = classifier.classify(&data, &context);
                    if classification.priority.should_transmit() {
                        let msg = encoder.encode(&data, &classification, &context);
                        encoded_total += msg.len();
                    }
                    context.observe(&data);
                }

                black_box((raw_total, encoded_total));
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_encoding,
    bench_decoding,
    bench_roundtrip,
    bench_classification,
    bench_context_operations,
    bench_compression_ratio,
);

criterion_main!(benches);
