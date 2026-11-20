const ESCAPE: u8 = b'=';
const ESCAPE_LEN: usize = 3;

#[must_use]
pub fn decode_quoted_printable(input: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(input.len());
    let mut lines = input.split(|byte| *byte == b'\n').peekable();

    while let Some(line) = lines.next() {
        let ends_line = lines.peek().is_some();
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let line = trim_trailing_space(line);

        let (line, soft_break) = match line.strip_suffix(&[ESCAPE]) {
            Some(content) => (content, true),
            None => (line, false),
        };

        let mut index = 0;
        while index < line.len() {
            match escaped_byte(&line[index..]) {
                Some(byte) => {
                    decoded.push(byte);
                    index += ESCAPE_LEN;
                }
                None => {
                    decoded.push(line[index]);
                    index += 1;
                }
            }
        }

        if ends_line && !soft_break {
            decoded.extend_from_slice(b"\r\n");
        }
    }

    decoded
}

#[must_use]
pub fn decode_q_encoded(input: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(input.len());
    let mut index = 0;

    while index < input.len() {
        match escaped_byte(&input[index..]) {
            Some(byte) => {
                decoded.push(byte);
                index += ESCAPE_LEN;
            }
            None => {
                decoded.push(match input[index] {
                    b'_' => b' ',
                    other => other,
                });
                index += 1;
            }
        }
    }

    decoded
}

fn escaped_byte(input: &[u8]) -> Option<u8> {
    let [ESCAPE, high, low, ..] = *input else {
        return None;
    };
    Some(hex_digit(high)? << 4 | hex_digit(low)?)
}

fn trim_trailing_space(line: &[u8]) -> &[u8] {
    let content_len = line
        .iter()
        .rposition(|byte| *byte != b' ' && *byte != b'\t')
        .map_or(0, |last| last + 1);
    &line[..content_len]
}

fn hex_digit(byte: u8) -> Option<u8> {
    char::from(byte)
        .to_digit(16)
        .and_then(|digit| u8::try_from(digit).ok())
}
