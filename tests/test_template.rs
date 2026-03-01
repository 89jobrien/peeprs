use peeprs::template::render_html;

#[test]
fn test_render_html_placeholder_replaced() {
    let html = render_html(5000);
    assert!(!html.contains("REFRESH_MS_PLACEHOLDER"));
    assert!(html.contains("const REFRESH_MS = 5000;"));
    assert!(html.contains("Agent Logs Dashboard"));
}

#[test]
fn test_render_html_doctype_present() {
    let html = render_html(10000);
    assert!(html.starts_with("<!doctype html>"));
}

#[test]
fn test_render_html_api_endpoint_referenced() {
    let html = render_html(10000);
    assert!(html.contains("/api/summary"));
}
