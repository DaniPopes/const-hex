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

fn test_buffer<const N: usize, const LEN: usize>(bytes: &[u8]) {
    if let Ok(bytes) = <[u8; N]>::try_from(bytes) {
        let mut buffer = const_hex::Buffer::<N, false>::new();
        let string = buffer.format(&bytes).to_string();
        assert_eq!(string.len(), bytes.len() * 2);
        assert_eq!(string.as_bytes(), buffer.as_byte_array::<LEN>());
        assert_eq!(string, buffer.as_str());
        assert_eq!(string, mk_expected(&bytes));

        let mut buffer = const_hex::Buffer::<N, true>::new();
        let prefixed = buffer.format(&bytes).to_string();
        assert_eq!(prefixed.len(), 2 + bytes.len() * 2);
        assert_eq!(prefixed, buffer.as_str());
        assert_eq!(prefixed, format!("0x{string}"));
    }
}

fuzz_target!(|input: &[u8]| {
    fuzz_encode(input);
    fuzz_decode(input);
});

fn fuzz_encode(input: &[u8]) {
    test_buffer::<8, 16>(input);
    test_buffer::<20, 40>(input);
    test_buffer::<32, 64>(input);
    test_buffer::<64, 128>(input);
    test_buffer::<128, 256>(input);

    let encoded = const_hex::encode(input);
    let expected = mk_expected(input);
    assert_eq!(encoded, expected);

    let decoded = const_hex::decode(&encoded).unwrap();
    assert_eq!(decoded, input);
}

fn fuzz_decode(input: &[u8]) {
    if let Ok(decoded) = const_hex::decode(input) {
        let prefix = if input.starts_with(b"0x") { 2 } else { 0 };
        let input_len = (input.len() - prefix) / 2;
        assert_eq!(decoded.len(), input_len);
    }
}
