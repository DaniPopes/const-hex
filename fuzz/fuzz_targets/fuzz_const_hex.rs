#![no_main]

use libfuzzer_sys::fuzz_target;
use std::io::Write;

fn mk_expected(bytes: &[u8]) -> String {
    let mut s = Vec::with_capacity(bytes.len() * 2);
    for i in bytes {
        write!(s, "{i:02x}").unwrap();
    }
    unsafe { String::from_utf8_unchecked(s) }
}

fn test_buffer<const N: usize>(bytes: &[u8]) {
    if let Ok(bytes) = <[u8; N]>::try_from(bytes) {
        let mut buffer = const_hex::Buffer::new();
        let string = buffer.format(&bytes);
        assert_eq!(string.len(), bytes.len() * 2);
        assert_eq!(string, &mk_expected(&bytes));
    }
}

fuzz_target!(|input: &[u8]| {
    test_buffer::<8>(input);
    test_buffer::<20>(input);
    test_buffer::<32>(input);
    test_buffer::<64>(input);
    test_buffer::<128>(input);

    let bytes = const_hex::encode(input);
    let expected = mk_expected(input);
    assert_eq!(bytes, expected);
});
