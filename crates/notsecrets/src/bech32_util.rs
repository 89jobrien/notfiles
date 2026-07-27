use crate::error::AgeError;

/// Decode a bech32 string into a 32-byte key, checking the expected HRP.
///
/// Age secret keys (`AGE-SECRET-KEY-...`) are conventionally uppercase but the
/// `bech32` crate only decodes lowercase, so identities pass `lowercase_input:
/// true`; recipients (`age1...`) are already lowercase.
pub(crate) fn decode_bech32_32(
    s: &str,
    expected_hrp: &str,
    lowercase_input: bool,
) -> Result<[u8; 32], AgeError> {
    let owned;
    let s = if lowercase_input {
        owned = s.to_lowercase();
        owned.as_str()
    } else {
        s
    };

    let (hrp, data) =
        bech32::decode(s).map_err(|e| AgeError::ParseError(format!("bech32 decode: {e}")))?;
    if hrp.as_str() != expected_hrp {
        return Err(AgeError::ParseError(format!(
            "expected hrp '{expected_hrp}', got '{}'",
            hrp.as_str()
        )));
    }
    data.try_into()
        .map_err(|_| AgeError::ParseError("key must be 32 bytes".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_bech32_32_roundtrips_and_checks_hrp() {
        let hrp = bech32::Hrp::parse("test").unwrap();
        let bytes = [7u8; 32];
        let encoded = bech32::encode::<bech32::Bech32>(hrp, &bytes).unwrap();

        let decoded = decode_bech32_32(&encoded, "test", false).unwrap();
        assert_eq!(decoded, bytes);

        let decoded_upper = decode_bech32_32(&encoded.to_uppercase(), "test", true).unwrap();
        assert_eq!(decoded_upper, bytes);

        assert!(decode_bech32_32(&encoded, "wrong", false).is_err());
    }
}
