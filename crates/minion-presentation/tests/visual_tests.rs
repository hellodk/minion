use minion_presentation::visual::svg_sanitizer::sanitize_svg;

#[test]
fn valid_svg_passes_through() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="10" height="10"/></svg>"#;
    assert!(sanitize_svg(input).unwrap().contains("<rect"));
}

#[test]
fn script_tag_stripped() {
    let input =
        r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script><rect/></svg>"#;
    let out = sanitize_svg(input).unwrap();
    assert!(!out.contains("script") && out.contains("<rect"));
}

#[test]
fn invalid_use_href_rejected() {
    let input =
        r#"<svg xmlns="http://www.w3.org/2000/svg"><use href="javascript:alert(1)"/></svg>"#;
    assert!(sanitize_svg(input).is_err());
}

#[test]
fn on_event_attr_stripped() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect onclick="evil()" width="10" height="10"/></svg>"#;
    assert!(!sanitize_svg(input).unwrap().contains("onclick"));
}

#[test]
fn fe_gaussian_blur_capped_at_20() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><filter><feGaussianBlur stdDeviation="999"/></filter></svg>"#;
    assert!(sanitize_svg(input).unwrap().contains("stdDeviation=\"20\""));
}
