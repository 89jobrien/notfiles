use notsecrets::format::{header_bytes_up_to_footer, parse_header, serialize_header};
use notsecrets::identities::{Header, Stanza};
use proptest::prelude::*;

fn stanza() -> impl Strategy<Value = Stanza> {
    (
        "[A-Za-z0-9-]{1,10}",
        prop::collection::vec("[A-Za-z0-9+/]{1,12}", 0..4),
        prop::collection::vec(any::<u8>(), 0..200),
    )
        .prop_map(|(tag, args, body)| Stanza { tag, args, body })
}

fn header() -> impl Strategy<Value = Header> {
    (
        prop::collection::vec(stanza(), 0..4),
        prop::collection::vec(any::<u8>(), 32),
    )
        .prop_map(|(recipients, mac)| Header { recipients, mac })
}

proptest! {
    /// serialize then parse returns the same header and consumes exactly the header bytes.
    #[test]
    fn header_round_trips(h in header(), payload in prop::collection::vec(any::<u8>(), 0..64)) {
        let mut bytes = serialize_header(&h);
        let header_len = bytes.len();
        bytes.extend_from_slice(&payload);

        let (parsed, offset) = parse_header(&bytes).expect("a serialized header parses");

        prop_assert_eq!(offset, header_len);
        prop_assert_eq!(parsed.mac, h.mac);
        prop_assert_eq!(parsed.recipients.len(), h.recipients.len());
        for (got, want) in parsed.recipients.iter().zip(&h.recipients) {
            prop_assert_eq!(&got.tag, &want.tag);
            prop_assert_eq!(&got.args, &want.args);
            prop_assert_eq!(&got.body, &want.body);
        }
    }

    /// Arbitrary bytes are rejected or parsed, never a panic.
    #[test]
    fn parse_header_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..300)) {
        let _ = parse_header(&bytes);
        let _ = header_bytes_up_to_footer(&bytes);
    }

    /// Same, but starting from a valid version line so the parser gets past its first check.
    #[test]
    fn parse_header_never_panics_after_a_valid_version_line(
        tail in prop::collection::vec(any::<u8>(), 0..300),
    ) {
        let mut bytes = b"age-encryption.org/v1\n".to_vec();
        bytes.extend_from_slice(&tail);
        let _ = parse_header(&bytes);
    }

    /// The MAC input is a prefix of the input that ends with the footer marker.
    #[test]
    fn mac_input_is_a_prefix_ending_at_the_footer(h in header()) {
        let bytes = serialize_header(&h);
        let prefix = header_bytes_up_to_footer(&bytes).expect("footer present");
        prop_assert!(bytes.starts_with(&prefix));
        prop_assert!(prefix.ends_with(b"--- "));
    }
}
