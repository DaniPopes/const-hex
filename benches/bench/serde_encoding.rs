use divan::Bencher;
use serde::Serializer;
use std::hint::black_box;

// Reuse the final JSON buffer to isolate encoding/serialization from output allocation.
#[divan::bench(args = [32, 128, 129, 256, 2046, 2048, 4096, 16384, 131072, 1048576])]
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
#[divan::bench(args = [32, 128, 129, 256, 2046, 2048, 4096, 16384, 131072, 1048576])]
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

#[divan::bench(args = [32, 128, 129, 256, 2046, 2048, 4096, 16384, 131072, 1048576])]
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

// Allocate inside the timed closure and return the Vec: Divan retains all 32
// outputs until the sample ends, preventing allocation reuse within the sample.
// Drops are outside timing; allocations can still be recycled between samples.
// PREALLOCATED=false includes Vec growth, true reserves the exact output size.
#[divan::bench(
    consts = [false, true],
    args = [32, 128, 129, 256, 2046, 2048, 4096, 16384, 131072, 1048576],
    sample_size = 32,
)]
fn fresh_buffered<const PREALLOCATED: bool>(b: Bencher, len: usize) {
    let data: Vec<_> = (0..len).map(|i| i as u8).collect();
    b.bench_local(|| {
        let mut output = if PREALLOCATED {
            Vec::with_capacity(black_box(2 * len + 4))
        } else {
            Vec::new()
        };
        const_hex::serialize(
            black_box(&data),
            &mut serde_json::Serializer::new(&mut output),
        )
        .unwrap();
        black_box(output)
    });
}

#[divan::bench(
    consts = [false, true],
    args = [32, 128, 129, 256, 2046, 2048, 4096, 16384, 131072, 1048576],
    sample_size = 32,
)]
fn fresh_unbuffered<const PREALLOCATED: bool>(b: Bencher, len: usize) {
    let data: Vec<_> = (0..len).map(|i| i as u8).collect();
    b.bench_local(|| {
        let mut output = if PREALLOCATED {
            Vec::with_capacity(black_box(2 * len + 4))
        } else {
            Vec::new()
        };
        serde_json::Serializer::new(&mut output)
            .collect_str(&format_args!("{:#}", const_hex::display(black_box(&data))))
            .unwrap();
        black_box(output)
    });
}

// Retain the temporary hex string too, preventing its allocator from recycling
// the same block for every serialization within a sample.
#[divan::bench(
    consts = [false, true],
    args = [32, 128, 129, 256, 2046, 2048, 4096, 16384, 131072, 1048576],
    sample_size = 32,
)]
fn fresh_stack_or_heap<const PREALLOCATED: bool>(b: Bencher, len: usize) {
    let data: Vec<_> = (0..len).map(|i| i as u8).collect();
    b.bench_local(|| {
        let mut output = if PREALLOCATED {
            Vec::with_capacity(black_box(2 * len + 4))
        } else {
            Vec::new()
        };
        let data = black_box(&data);
        let mut serializer = serde_json::Serializer::new(&mut output);
        let encoded = if data.len() <= 128 {
            const_hex::serialize(data, &mut serializer).unwrap();
            None
        } else {
            let encoded = const_hex::encode_prefixed(data);
            serializer.serialize_str(&encoded).unwrap();
            Some(encoded)
        };
        black_box((output, encoded))
    });
}

// A fresh fixed stack destination, including initialization in timing.
// MODE: 0 = buffered streaming, 1 = direct streaming, 2 = old stack/heap encoder.
// 2 * 2046 + 4 fills the destination exactly, including quotes and prefix.
#[divan::bench(
    consts = [0, 1, 2],
    args = [32, 128, 129, 256, 2046],
    sample_size = 32,
)]
fn stack_destination<const MODE: u8>(b: Bencher, len: usize) {
    let data: Vec<_> = (0..len).map(|i| i as u8).collect();
    b.bench_local(|| {
        let mut output = [0u8; 4096];
        let mut remaining = output.as_mut_slice();
        let data = black_box(&data);
        let mut serializer = serde_json::Serializer::new(&mut remaining);
        let encoded = match MODE {
            0 => {
                const_hex::serialize(data, &mut serializer).unwrap();
                None
            }
            1 => {
                serializer
                    .collect_str(&format_args!("{:#}", const_hex::display(data)))
                    .unwrap();
                None
            }
            2 if data.len() <= 128 => {
                const_hex::serialize(data, &mut serializer).unwrap();
                None
            }
            2 => {
                let encoded = const_hex::encode_prefixed(data);
                serializer.serialize_str(&encoded).unwrap();
                Some(encoded)
            }
            _ => unreachable!(),
        };
        let written = 4096 - remaining.len();
        black_box(&output[..written]);
        // Keep heap intermediates alive until the end of the sample as well.
        black_box(encoded)
    });
}
