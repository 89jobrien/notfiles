use notfiles::ignore::IgnoreMatcher;
use proptest::prelude::*;
use std::path::PathBuf;

/// A path component that can never equal the ignored name (different alphabet).
fn other_component() -> impl Strategy<Value = String> {
    "[A-Z][A-Z0-9]{0,6}"
}

fn literal_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9]{0,6}"
}

fn join(parts: &[String]) -> PathBuf {
    parts.iter().collect()
}

proptest! {
    /// A literal pattern ignores any path that has a component with exactly that name.
    #[test]
    fn literal_pattern_matches_at_any_depth(
        name in literal_name(),
        before in prop::collection::vec(other_component(), 0..4),
        after in prop::collection::vec(other_component(), 0..4),
    ) {
        let matcher = IgnoreMatcher::new(&[name.as_str()]).expect("literal pattern compiles");
        let mut parts = before;
        parts.push(name);
        parts.extend(after);

        prop_assert!(matcher.is_ignored(&join(&parts)));
    }

    /// ...and ignores nothing else.
    #[test]
    fn literal_pattern_leaves_other_paths_alone(
        name in literal_name(),
        parts in prop::collection::vec(other_component(), 1..5),
    ) {
        let matcher = IgnoreMatcher::new(&[name.as_str()]).expect("literal pattern compiles");

        prop_assert!(!matcher.is_ignored(&join(&parts)));
    }

    /// Adding patterns can only ignore more, never less.
    #[test]
    fn more_patterns_never_ignore_less(
        a in literal_name(),
        b in literal_name(),
        parts in prop::collection::vec(literal_name(), 1..5),
    ) {
        let path = join(&parts);
        let one = IgnoreMatcher::new(&[a.as_str()]).expect("compiles");
        let two = IgnoreMatcher::new(&[a.as_str(), b.as_str()]).expect("compiles");

        prop_assert!(!one.is_ignored(&path) || two.is_ignored(&path));
    }

    /// Arbitrary pattern text either compiles or is rejected with an error, never a panic;
    /// a compiled matcher can be queried with arbitrary paths.
    #[test]
    fn arbitrary_patterns_never_panic(pattern in "\\PC{0,20}", path in "\\PC{0,30}") {
        if let Ok(matcher) = IgnoreMatcher::new(&[pattern.as_str()]) {
            let _ = matcher.is_ignored(&PathBuf::from(path));
        }
    }
}
