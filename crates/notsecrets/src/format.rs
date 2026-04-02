use crate::error::AgeError;
use crate::identities::{Header, Stanza};
use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};

const VERSION_LINE: &[u8] = b"age-encryption.org/v1\n";
const FOOTER_PREFIX: &[u8] = b"--- ";

/// Parse an age binary file's header.
///
/// Returns `(Header, payload_offset)` where `payload_offset` is the byte
/// index into `input` where the payload (nonce + ciphertext) begins.
pub fn parse_header(input: &[u8]) -> Result<(Header, usize), AgeError> {
    if !input.starts_with(VERSION_LINE) {
        return Err(AgeError::ParseError(
            "missing age version line".to_string(),
        ));
    }

    let mut pos = VERSION_LINE.len();
    let mut recipients: Vec<Stanza> = Vec::new();

    loop {
        if pos >= input.len() {
            return Err(AgeError::ParseError("unexpected end of header".to_string()));
        }

        // Check for footer line: "--- <base64-mac>\n"
        if input[pos..].starts_with(FOOTER_PREFIX) {
            let line_end = input[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .ok_or_else(|| AgeError::ParseError("footer line not terminated".to_string()))?;
            let mac_b64 = &input[pos + FOOTER_PREFIX.len()..pos + line_end];
            let mac = STANDARD_NO_PAD
                .decode(mac_b64)
                .map_err(|e| AgeError::ParseError(format!("footer MAC base64: {e}")))?;
            let payload_offset = pos + line_end + 1;
            return Ok((Header { recipients, mac }, payload_offset));
        }

        // Must be a stanza line: "-> <tag> [args...]\n"
        if !input[pos..].starts_with(b"-> ") {
            return Err(AgeError::ParseError(format!(
                "expected stanza or footer at byte {pos}"
            )));
        }

        let line_end = input[pos..]
            .iter()
            .position(|&b| b == b'\n')
            .ok_or_else(|| AgeError::ParseError("stanza header line not terminated".to_string()))?;

        let header_line = std::str::from_utf8(&input[pos + 3..pos + line_end])
            .map_err(|e| AgeError::ParseError(format!("stanza line UTF-8: {e}")))?;

        let mut parts = header_line.split(' ');
        let tag = parts
            .next()
            .ok_or_else(|| AgeError::ParseError("stanza missing tag".to_string()))?
            .to_string();
        let args: Vec<String> = parts.map(str::to_string).collect();

        pos += line_end + 1;

        // Read stanza body: base64 lines until an empty line
        let mut body_b64 = String::new();
        loop {
            if pos >= input.len() {
                return Err(AgeError::ParseError(
                    "unexpected end during stanza body".to_string(),
                ));
            }
            let line_end = input[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .ok_or_else(|| {
                    AgeError::ParseError("stanza body line not terminated".to_string())
                })?;
            let line = &input[pos..pos + line_end];
            pos += line_end + 1;
            if line.is_empty() {
                break;
            }
            body_b64.push_str(
                std::str::from_utf8(line)
                    .map_err(|e| AgeError::ParseError(format!("body UTF-8: {e}")))?,
            );
        }

        let body = if body_b64.is_empty() {
            Vec::new()
        } else {
            STANDARD_NO_PAD
                .decode(&body_b64)
                .map_err(|e| AgeError::ParseError(format!("stanza body base64: {e}")))?
        };

        recipients.push(Stanza { tag, args, body });
    }
}

/// Serialize a `Header` to bytes.
///
/// Writes version line, all stanzas (with body split into 64-char base64 lines),
/// and the `--- <base64-mac>` footer. Does NOT write the payload.
pub fn serialize_header(header: &Header) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(VERSION_LINE);

    for stanza in &header.recipients {
        out.extend_from_slice(b"-> ");
        out.extend_from_slice(stanza.tag.as_bytes());
        for arg in &stanza.args {
            out.push(b' ');
            out.extend_from_slice(arg.as_bytes());
        }
        out.push(b'\n');

        if stanza.body.is_empty() {
            out.push(b'\n');
        } else {
            let encoded = STANDARD_NO_PAD.encode(&stanza.body);
            for chunk in encoded.as_bytes().chunks(64) {
                out.extend_from_slice(chunk);
                out.push(b'\n');
            }
            // Terminating empty line after body
            out.push(b'\n');
        }
    }

    // Footer
    out.extend_from_slice(FOOTER_PREFIX);
    out.extend_from_slice(STANDARD_NO_PAD.encode(&header.mac).as_bytes());
    out.push(b'\n');

    out
}

/// Return the header bytes up to and including `"--- "` (not including the MAC value).
///
/// Used for MAC computation.
pub fn header_bytes_up_to_footer(input: &[u8]) -> Result<Vec<u8>, AgeError> {
    let footer_pos = input
        .windows(FOOTER_PREFIX.len())
        .position(|w| w == FOOTER_PREFIX)
        .ok_or_else(|| AgeError::ParseError("footer not found".to_string()))?;
    Ok(input[..footer_pos + FOOTER_PREFIX.len()].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::{Header, Stanza};

    fn minimal_age_file() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"age-encryption.org/v1\n");
        out.extend_from_slice(b"-> X25519 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\n");
        // empty stanza body
        out.extend_from_slice(b"\n");
        // MAC line: "--- " + base64(32 zero bytes) -- note: STANDARD_NO_PAD encodes 32 zeros as 43 chars
        out.extend_from_slice(b"--- AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n");
        // payload: 16-byte nonce + 16-byte ciphertext placeholder
        out.extend_from_slice(&[0x01u8; 16]);
        out.extend_from_slice(&[0x02u8; 16]);
        out
    }

    #[test]
    fn parse_minimal_header_roundtrip() {
        let input = minimal_age_file();
        let (header, payload_offset) = parse_header(&input).expect("parse should succeed");
        assert_eq!(header.recipients.len(), 1);
        assert_eq!(header.recipients[0].tag, "X25519");
        assert_eq!(header.recipients[0].args.len(), 1);
        assert!(header.recipients[0].body.is_empty());
        // payload offset should point to the 32 bytes at the end
        assert_eq!(&input[payload_offset..], {
            let mut v = vec![0x01u8; 16];
            v.extend_from_slice(&[0x02u8; 16]);
            v
        }.as_slice());
    }

    #[test]
    fn serialize_header_roundtrip() {
        let stanza = Stanza {
            tag: "X25519".to_string(),
            args: vec!["dGVzdA".to_string()],
            body: vec![0xde, 0xad, 0xbe, 0xef],
        };
        let header = Header {
            recipients: vec![stanza],
            mac: vec![0u8; 32],
        };
        let serialized = serialize_header(&header);
        // Append dummy payload so parse_header can find the end
        let mut with_payload = serialized.clone();
        with_payload.extend_from_slice(&[0u8; 32]);
        let (parsed, _) = parse_header(&with_payload).expect("round-trip parse should succeed");
        assert_eq!(parsed.recipients.len(), 1);
        assert_eq!(parsed.recipients[0].tag, "X25519");
        assert_eq!(parsed.recipients[0].body, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn parse_missing_version_line_returns_error() {
        let input = b"not-age-encryption\n-> X25519 arg\n\n--- AAAA\n";
        let result = parse_header(input);
        assert!(result.is_err(), "expected ParseError for missing version line");
    }

    #[test]
    fn parse_truncated_before_footer_returns_error() {
        let input = b"age-encryption.org/v1\n-> X25519 arg\n";
        let result = parse_header(input);
        assert!(result.is_err(), "expected ParseError for truncated header");
    }

    #[test]
    fn parse_empty_input_returns_error() {
        let result = parse_header(b"");
        assert!(result.is_err());
    }

    #[test]
    fn stanza_body_split_across_multiple_64char_lines() {
        // 49 bytes -> base64 no-pad = 66 chars -> must split: 64 + 2
        let body = vec![0xabu8; 49];
        let stanza = Stanza {
            tag: "scrypt".to_string(),
            args: vec!["salt".to_string(), "18".to_string()],
            body: body.clone(),
        };
        let header = Header { recipients: vec![stanza], mac: vec![0u8; 32] };
        let serialized = serialize_header(&header);
        let mut with_payload = serialized;
        with_payload.extend_from_slice(&[0u8; 32]);
        let (parsed, _) = parse_header(&with_payload).unwrap();
        assert_eq!(parsed.recipients[0].body, body);
    }

    #[test]
    fn header_bytes_up_to_footer_excludes_mac() {
        let input = minimal_age_file();
        let bytes = header_bytes_up_to_footer(&input).expect("should find footer");
        // Must end with "--- " (not include the MAC value)
        assert!(bytes.ends_with(b"--- "), "header_bytes must end at '--- '");
    }
}
