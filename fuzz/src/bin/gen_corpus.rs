//! Writes the committed seed corpus. Seeds are deterministic except for the ephemeral
//! key inside each ciphertext, so regenerate only when the format changes.

use notsecrets::format::serialize_header;
use notsecrets::{Decryptor, Encryptor, Header, Stanza, X25519Identity, X25519Recipient};
use std::fs;
use std::path::Path;
use x25519_dalek::{PublicKey, StaticSecret};

fn write(dir: &str, name: &str, bytes: &[u8]) {
    let dir = Path::new("fuzz/corpus").join(dir);
    fs::create_dir_all(&dir).expect("create corpus dir");
    fs::write(dir.join(name), bytes).expect("write seed");
}

fn main() {
    let secret = StaticSecret::from([7u8; 32]);
    let recipient = X25519Recipient::from_public_key(PublicKey::from(&secret));
    let identity = X25519Identity::from_static_secret(secret);

    let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)]).expect("encryptor");
    let empty = encryptor.encrypt(b"").expect("encrypt empty");
    let short = encryptor.encrypt(b"hello fuzzing").expect("encrypt short");
    let long = encryptor.encrypt(&[0x5a; 70_000]).expect("encrypt long");

    // A corpus of undecryptable seeds would never get the fuzzer past the MAC check.
    let decryptor = Decryptor::with_identities(vec![Box::new(identity)]);
    assert_eq!(
        decryptor.decrypt(&short).expect("seed decrypts"),
        b"hello fuzzing"
    );
    assert!(decryptor.decrypt(&empty).expect("seed decrypts").is_empty());
    assert_eq!(
        decryptor.decrypt(&long).expect("seed decrypts").len(),
        70_000
    );

    for (name, bytes) in [
        ("valid_empty", &empty),
        ("valid_short", &short),
        ("valid_long", &long),
    ] {
        write("fuzz_decrypt", name, bytes);
        write("fuzz_parse_header", name, bytes);
        write("fuzz_header_bytes_up_to_footer", name, bytes);
    }

    let header = Header {
        recipients: vec![Stanza {
            tag: "X25519".into(),
            args: vec!["AAAA".into()],
            body: vec![1, 2, 3],
        }],
        mac: vec![0; 32],
    };
    let bare = serialize_header(&header);
    write("fuzz_parse_header", "header_only", &bare);
    write("fuzz_header_bytes_up_to_footer", "header_only", &bare);

    let truncated = &short[..short.len() / 2];
    write("fuzz_decrypt", "truncated", truncated);
    write("fuzz_parse_header", "truncated", truncated);

    write(
        "fuzz_parse_header",
        "version_only",
        b"age-encryption.org/v1\n",
    );
    write("fuzz_parse_header", "empty", b"");
}
