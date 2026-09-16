use divan::Bencher;
use serde::Serializer;
use std::hint::black_box;

// Reuse the final JSON buffer to isolate encoding/serialization from output allocation.
#[divan::bench(args = [32, 128, 129, 256, 2048, 4096, 16384, 131072, 1048576])]
fn buffered(b: Bencher, len: usize) {
    let data: Vec<_> = (0..len).map(|i| i as u8).collect();
    let mut output = Vec::with_capacity(2 * len + 4);
    b.bench_local(|| {
        output.clear();
        const_hex::serialize(
            black_box(&data),
            &mut serde_json::Serializer::new(&mut output),
        )
        .unwrap();
        black_box(&output);
    });
}

// The stack/heap implementation introduced by const-hex PR #51.
#[divan::bench(args = [32, 128, 129, 256, 2048, 4096, 16384, 131072, 1048576])]
fn stack_or_heap(b: Bencher, len: usize) {
    let data: Vec<_> = (0..len).map(|i| i as u8).collect();
    let mut output = Vec::with_capacity(2 * len + 4);
    b.bench_local(|| {
        output.clear();
        let data = black_box(&data);
        let mut serializer = serde_json::Serializer::new(&mut output);
        if data.len() <= 128 {
            const_hex::serialize(data, &mut serializer).unwrap();
        } else {
            serializer
                .serialize_str(&const_hex::encode_prefixed(data))
                .unwrap();
        }
        black_box(&output);
    });
}

#[divan::bench(args = [32, 128, 129, 256, 2048, 4096, 16384, 131072, 1048576])]
fn unbuffered(b: Bencher, len: usize) {
    let data: Vec<_> = (0..len).map(|i| i as u8).collect();
    let mut output = Vec::with_capacity(2 * len + 4);
    b.bench_local(|| {
        output.clear();
        serde_json::Serializer::new(&mut output)
            .collect_str(&format_args!("{:#}", const_hex::display(black_box(&data))))
            .unwrap();
        black_box(&output);
    });
}
