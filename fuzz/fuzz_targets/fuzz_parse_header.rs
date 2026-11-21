#![no_main]
use libfuzzer_sys::fuzz_target;
use notsecrets::format::parse_header;

fuzz_target!(|data: &[u8]| {
    // parse_header must never panic on arbitrary bytes, and a success must describe a real
    // prefix of the input.
    if let Ok((_header, payload_offset)) = parse_header(data) {
        assert!(payload_offset <= data.len());
        assert!(data.starts_with(b"age-encryption.org/v1\n"));
    }
});
