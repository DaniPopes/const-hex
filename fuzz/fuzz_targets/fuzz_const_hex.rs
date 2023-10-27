#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    const_hex::fuzzing::fuzz(data).unwrap();
});
