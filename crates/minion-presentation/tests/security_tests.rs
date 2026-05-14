use minion_presentation::security::ssrf_guard::validate_url;

#[test]
fn accepts_valid_public_https() {
    let result = validate_url("https://example.com/page");
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    let u = result.unwrap();
    assert_eq!(u.scheme(), "https");
}
#[test]
fn accepts_valid_public_http() {
    let result = validate_url("http://example.com/page");
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
}
#[test]
fn rejects_ftp_scheme() {
    let result = validate_url("ftp://example.com/file.txt");
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(msg.contains("scheme"), "error should mention scheme: {msg}");
}
#[test]
fn rejects_file_scheme() {
    let result = validate_url("file:///etc/passwd");
    assert!(result.is_err());
}
#[test]
fn rejects_localhost_loopback() {
    let result = validate_url("http://localhost/admin");
    assert!(result.is_err(), "localhost must be blocked");
}
#[test]
fn rejects_127_direct() {
    let result = validate_url("http://127.0.0.1/anything");
    assert!(result.is_err());
}
#[test]
fn rejects_private_10_block() {
    let result = validate_url("http://10.0.0.1/internal");
    assert!(result.is_err());
}
#[test]
fn rejects_private_192_168_block() {
    let result = validate_url("http://192.168.1.100/router");
    assert!(result.is_err());
}
#[test]
fn rejects_private_172_16_block() {
    let result = validate_url("http://172.16.5.10/service");
    assert!(result.is_err());
}
#[test]
fn rejects_link_local_169_254() {
    let result = validate_url("http://169.254.169.254/metadata");
    assert!(result.is_err(), "cloud metadata endpoint must be blocked");
}
#[test]
fn rejects_invalid_url_string() {
    let result = validate_url("not a url at all");
    assert!(result.is_err());
}
#[test]
fn rejects_empty_string() {
    let result = validate_url("");
    assert!(result.is_err());
}
#[test]
fn max_redirect_constant_is_three() {
    assert_eq!(minion_presentation::security::ssrf_guard::MAX_REDIRECTS, 3);
}
