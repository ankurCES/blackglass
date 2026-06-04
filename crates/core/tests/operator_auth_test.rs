use blackglass_core::operator_auth::OperatorAuth;
use std::fs;
use tempfile::tempdir;

#[test]
fn verify_returns_ok_when_token_matches() {
    let dir = tempdir().unwrap();
    let token_path = dir.path().join("operator.token");
    fs::write(&token_path, "secret-token-abc123\n").unwrap();
    fs::set_permissions(&token_path, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    let auth = OperatorAuth::new(&token_path);
    assert!(auth.verify(b"secret-token-abc123\n").is_ok());
}

#[test]
fn verify_returns_err_on_wrong_token() {
    let dir = tempdir().unwrap();
    let token_path = dir.path().join("operator.token");
    fs::write(&token_path, "secret-token-abc123\n").unwrap();
    // File must be 0600 so verify reaches the token-comparison path
    // instead of short-circuiting with TokenFileBadMode.
    fs::set_permissions(&token_path, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
    let auth = OperatorAuth::new(&token_path);
    let err = auth.verify(b"wrong-token\n").unwrap_err();
    assert!(err.to_string().contains("auth"));
}

#[test]
fn verify_returns_err_when_token_file_missing() {
    let dir = tempdir().unwrap();
    let token_path = dir.path().join("does-not-exist");
    let auth = OperatorAuth::new(&token_path);
    assert!(auth.verify(b"any\n").is_err());
}

#[test]
fn verify_returns_err_when_token_file_is_world_readable() {
    // Defense in depth: the token file must be 0600. If it's 0644, refuse to use it.
    let dir = tempdir().unwrap();
    let token_path = dir.path().join("operator.token");
    fs::write(&token_path, "secret-token\n").unwrap();
    fs::set_permissions(&token_path, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
    let auth = OperatorAuth::new(&token_path);
    let err = auth.verify(b"secret-token\n").unwrap_err();
    assert!(err.to_string().contains("mode"));
}
