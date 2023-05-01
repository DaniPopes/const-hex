#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
enum Input {
    _20([u8; 20]),
    _32([u8; 32]),
}

macro_rules! test {
    ($val:expr) => {
        match $val {
            val => {
                let mut buffer = const_hex::Buffer::new();
                let string = buffer.format(&val);
                assert_eq!(string.len(), val.len() * 2);
                let expected = val.iter().map(|b| format!("{b:02x}")).collect::<String>();
                assert_eq!(string, expected);
            }
        }
    };
}

fuzz_target!(|input: Input| {
    match input {
        Input::_20(val) => test!(val),
        Input::_32(val) => test!(val),
    }
});
