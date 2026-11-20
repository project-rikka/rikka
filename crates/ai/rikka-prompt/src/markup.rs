#[must_use]
pub fn escape_attribute(text: &str) -> String {
    text.replace('&', "&amp;").replace('"', "&quot;")
}
