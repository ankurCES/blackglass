use blackglass_engagement::{Engagement, Target, TargetKind};

#[test]
fn ip_target_is_allowed() {
    let mut e = Engagement::new("eng-1", "Lab test 2026-06-03", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    e.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    assert!(e.allows("10.0.0.5"));
    assert!(!e.allows("10.0.0.6"));
}

#[test]
fn cidr_target_is_allowed() {
    let mut e = Engagement::new("eng-2", "Subnet test", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    e.add_target(Target { value: "10.0.0.0/24".into(), kind: TargetKind::Cidr });
    assert!(e.allows("10.0.0.1"));
    assert!(e.allows("10.0.0.254"));
    assert!(!e.allows("10.0.1.1"));
}

#[test]
fn hostname_target_is_allowed() {
    let mut e = Engagement::new("eng-3", "Web test", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    e.add_target(Target { value: "lab.example.com".into(), kind: TargetKind::Hostname });
    assert!(e.allows("lab.example.com"));
    assert!(!e.allows("other.example.com"));
}

#[test]
fn empty_engagement_allows_nothing() {
    let e = Engagement::new("eng-empty", "Empty", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    assert!(!e.allows("10.0.0.1"));
    assert!(!e.allows("anything.example.com"));
}

#[test]
fn mixed_targets_each_match_their_own_kind() {
    let mut e = Engagement::new("eng-mix", "Mixed", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    e.add_target(Target { value: "10.0.0.5".into(), kind: TargetKind::Ip });
    e.add_target(Target { value: "192.168.1.0/24".into(), kind: TargetKind::Cidr });
    e.add_target(Target { value: "lab.example.com".into(), kind: TargetKind::Hostname });
    assert!(e.allows("10.0.0.5"));
    assert!(e.allows("192.168.1.42"));
    assert!(e.allows("lab.example.com"));
    assert!(!e.allows("10.0.0.6"));
    assert!(!e.allows("192.168.2.1"));
}

#[test]
fn engagement_round_trips_through_toml() {
    let mut e = Engagement::new("eng-rt", "RT", "2026-06-01T00:00:00Z", "2026-06-30T00:00:00Z");
    e.add_target(Target { value: "10.0.0.0/24".into(), kind: TargetKind::Cidr });
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("eng.toml");
    std::fs::write(&p, toml::to_string(&e).unwrap()).unwrap();
    let s = std::fs::read_to_string(&p).unwrap();
    let e2: Engagement = toml::from_str(&s).unwrap();
    assert!(e2.allows("10.0.0.7"));
}
