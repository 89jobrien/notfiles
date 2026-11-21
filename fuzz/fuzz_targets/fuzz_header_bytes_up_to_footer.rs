#![no_main]
use libfuzzer_sys::fuzz_target;
use notsecrets::format::header_bytes_up_to_footer;

fuzz_target!(|data: &[u8]| {
    // The MAC input is always a prefix of the file ending at the footer marker.
    if let Ok(prefix) = header_bytes_up_to_footer(data) {
        assert!(data.starts_with(&prefix));
        assert!(prefix.ends_with(b"--- "));
    }
});
