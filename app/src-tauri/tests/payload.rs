use blackglass_app::build_confirm_resolve;

#[test]
fn confirm_resolve_payload_is_valid_jsonrpc() {
    let payload = build_confirm_resolve("018f3b1c-7e2a-7c2e-bf3e-1c0a2b3c4d5e", "allow");
    assert_eq!(payload["jsonrpc"], "2.0");
    assert_eq!(payload["method"], "confirm.resolve");
    assert_eq!(payload["params"]["id"], "018f3b1c-7e2a-7c2e-bf3e-1c0a2b3c4d5e");
    assert_eq!(payload["params"]["decision"], "allow");
}
