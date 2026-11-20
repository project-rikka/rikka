use std::fmt;

#[derive(Clone, Copy, Debug)]
pub struct Escaped<'a>(pub &'a str);

impl fmt::Display for Escaped<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut rest = self.0;
        while let Some(position) = rest.find(['&', '<', '>', '"', '\'']) {
            formatter.write_str(&rest[..position])?;
            formatter.write_str(match rest.as_bytes()[position] {
                b'&' => "&amp;",
                b'<' => "&lt;",
                b'>' => "&gt;",
                b'"' => "&quot;",
                _ => "&#39;",
            })?;
            rest = &rest[position + 1..];
        }
        formatter.write_str(rest)
    }
}
