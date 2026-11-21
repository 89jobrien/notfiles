#![no_main]
use libfuzzer_sys::fuzz_target;
use notsecrets::{Decryptor, X25519Identity};
use x25519_dalek::StaticSecret;

/// Same key as `gen_corpus`, so seeds are valid ciphertexts the fuzzer can mutate past
/// the header and MAC checks.
const SECRET: [u8; 32] = [7; 32];

fuzz_target!(|data: &[u8]| {
    let identity = X25519Identity::from_static_secret(StaticSecret::from(SECRET));
    let decryptor = Decryptor::with_identities(vec![Box::new(identity)]);
    // Decryption of arbitrary bytes must fail cleanly, never panic.
    let _ = decryptor.decrypt(data);
});
