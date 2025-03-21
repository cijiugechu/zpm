use rstest::rstest;

use crate::{Version, VersionRc};

#[rstest]
#[case("1.2.3", Version { major: 1, minor: 2, patch: 3, rc: None })]
#[case("1.2.3-rc", Version { major: 1, minor: 2, patch: 3, rc: Some(vec![VersionRc::String("rc".to_string())]) })]
#[case("1.2.3-rc.1", Version { major: 1, minor: 2, patch: 3, rc: Some(vec![VersionRc::String("rc".to_string()), VersionRc::Number(1)]) })]
#[case("1.2.3-rc.1.32a", Version { major: 1, minor: 2, patch: 3, rc: Some(vec![VersionRc::String("rc".to_string()), VersionRc::Number(1), VersionRc::String("32a".to_string())]) })]
#[case("5.11.0-next.1603014861.18546659943e6c5744ce67403b1c78c1993ccf84", Version { major: 5, minor: 11, patch: 0, rc: Some(vec![VersionRc::String("next".to_string()), VersionRc::Number(1603014861), VersionRc::String("18546659943e6c5744ce67403b1c78c1993ccf84".to_string())]) })]
fn test_version_parse(#[case] version: Version, #[case] expected: Version) {
    assert_eq!(version, expected);
}

#[rstest]
#[case("1.2.3", "1.2.4")]
#[case("1.2.3", "1.3.0")]
#[case("1.2.3", "2.0.0")]
#[case("1.2.3-rc.1", "1.2.3")]
fn test_version_lt(#[case] left: Version, #[case] right: Version) {
    assert!(left < right);
}

#[rstest]
#[case("1.2.0", "1.2.1-0")]
#[case("1.2.9", "1.2.10-0")]
#[case("1.2.0-42", "1.2.0-43")]
#[case("1.2.0-rc.1", "1.2.0-rc.2")]
#[case("1.2.0-rc", "1.2.0-rd")]
#[case("1.0.0-x-y-z.--", "1.0.0-x-y-z.-0")]
#[case("1.0.0-x-y-z.-", "1.0.0-x-y-z.a")]
fn test_version_next_immediate(#[case] left: Version, #[case] right: Version) {
    assert_eq!(left.next_immediate_spec(), right);
}