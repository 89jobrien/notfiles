use notsecrets::{Decryptor, Encryptor, X25519Identity, X25519Recipient};
use rand::rngs::OsRng;
use x25519_dalek::{PublicKey, StaticSecret};

fn main() {
    // 1. Generate a fresh keypair
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    let identity = X25519Identity::from_static_secret(secret);
    let recipient = X25519Recipient::from_public_key(public);

    println!("Recipient: {}", recipient.to_bech32());
    println!("Identity:  {}", identity.to_bech32());

    // 2. Build plaintext from env
    let plaintext = [
        (
            "ANTHROPIC_API_KEY",
            std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
        ),
        (
            "OPENAI_API_KEY",
            std::env::var("OPENAI_API_KEY").unwrap_or_default(),
        ),
        (
            "GEMINI_API_KEY",
            std::env::var("GEMINI_API_KEY").unwrap_or_default(),
        ),
    ]
    .iter()
    .map(|(k, v)| format!("{k}={v}"))
    .collect::<Vec<_>>()
    .join("\n");

    let plaintext = plaintext.as_bytes();

    // 3. Encrypt
    let encryptor = Encryptor::with_recipients(vec![Box::new(recipient)]).unwrap();
    let ciphertext = encryptor.encrypt(plaintext).unwrap();

    println!("\nCiphertext: {} bytes", ciphertext.len());
    println!(
        "Header:     {}",
        std::str::from_utf8(&ciphertext[..22]).unwrap()
    );

    // 4. Write to file
    let out_path = "/tmp/secrets.age";
    std::fs::write(out_path, &ciphertext).unwrap();
    println!("Written to: {out_path}");

    // 5. Decrypt back
    let decryptor = Decryptor::with_identities(vec![Box::new(identity)]);
    let decrypted = decryptor.decrypt(&ciphertext).unwrap();
    assert_eq!(decrypted, plaintext);

    println!("\nDecrypted:\n{}", std::str::from_utf8(&decrypted).unwrap());
    println!("\n✓ Round-trip success");
}
