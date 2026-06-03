// crates/profile/tests/load.rs
use blackglass_profile::{Profile, ProfileError, Tier};

#[test]
fn loads_analyst_profile_from_toml() {
    let toml = r#"
        name = "analyst"
        tier = "analyst"
        allowed_domains = ["core", "osint", "packets", "audit"]
        allowed_action_classes = ["read_only"]
    "#;
    let p = Profile::parse(toml).unwrap();
    assert_eq!(p.name, "analyst");
    assert_eq!(p.tier, Tier::Analyst);
    assert_eq!(p.allowed_domains, vec!["core", "osint", "packets", "audit"]);
    assert_eq!(p.allowed_action_classes, vec!["read_only"]);
}

#[test]
fn rejects_unknown_tier() {
    let toml = "name = \"x\"\ntier = \"god_mode\"\nallowed_domains = []\nallowed_action_classes = []\n";
    let err = Profile::parse(toml).unwrap_err();
    assert!(matches!(err, ProfileError::UnknownTier(_)));
}

#[test]
fn gate1_allows_only_listed_domain() {
    let p = Profile::analyst_default();
    assert!(p.allows_domain("osint"));
    assert!(!p.allows_domain("exploit"));
    assert!(!p.allows_domain("phish"));
}

#[test]
fn gate1_allows_only_listed_action_class() {
    let p = Profile::analyst_default();
    assert!(p.allows_action_class("read_only"));
    assert!(!p.allows_action_class("transmit"));
    assert!(!p.allows_action_class("credential_dump"));
}

use proptest::prelude::*;

proptest! {
    #[test]
    fn parse_never_panics(s in ".*") {
        let _ = Profile::parse(&s);
    }

    #[test]
    fn allowed_set_behaves_as_set(s in "[a-z]{1,8}") {
        let mut p = Profile::analyst_default();
        let was = p.allows_domain(&s);
        p.allowed_domains.push(s.clone());
        prop_assert!(p.allows_domain(&s));
        prop_assert!(was == matches!(s.as_str(), "core" | "osint" | "packets" | "audit"));
    }
}
