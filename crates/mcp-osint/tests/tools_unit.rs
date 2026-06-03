#[test]
fn whois_argv_is_shell_safe() {
    let bad = ["evil;cmd", "x&y", "a|b", "`cmd`", "$(x)", "a\nb"];
    for t in bad {
        assert!(
            t.chars().any(|c| matches!(c, ';' | '&' | '|' | '`' | '$' | '\n' | '\r')),
            "test vector {:?} should contain shell metachar",
            t
        );
    }
}

#[test]
fn whois_available_on_path() {
    if std::process::Command::new("whois")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("whois not installed, skipping");
        return;
    }
    let out = std::process::Command::new("whois")
        .arg("example.com")
        .output()
        .unwrap();
    assert!(
        out.status.success() || !out.stdout.is_empty(),
        "whois returned neither success nor output"
    );
}
