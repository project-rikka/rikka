use crate::mime::headers::{Headers, field_name_end};
use crate::mime::parameters::{ContentDisposition, ContentType};
use crate::mime::part::{MimePart, Multipart, PartBody, Protection};
use crate::mime::transfer;

const MAX_DEPTH: usize = 48;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefaultContentType {
    Text,
    Message,
}

pub(crate) fn parse(raw: &[u8], default_type: DefaultContentType, depth: usize) -> MimePart {
    let (head, body) = split(raw);
    let headers = Headers::parse(head);

    let mut content_type = headers
        .unfolded("content-type")
        .and_then(|value| ContentType::parse(&value))
        .unwrap_or_else(|| match default_type {
            DefaultContentType::Text => ContentType::new("text", "plain"),
            DefaultContentType::Message => ContentType::new("message", "rfc822"),
        });
    let disposition = headers
        .unfolded("content-disposition")
        .and_then(|value| ContentDisposition::parse(&value));
    let encoding = headers.unfolded("content-transfer-encoding");

    let body = if content_type.is_multipart() {
        let parsed = (depth < MAX_DEPTH)
            .then(|| multipart(&content_type, body, depth))
            .flatten();
        parsed.unwrap_or_else(|| {
            content_type = ContentType::new("text", "plain");
            PartBody::Leaf(transfer::decode(body, encoding.as_deref()))
        })
    } else if content_type.is_message() && depth < MAX_DEPTH {
        let raw = transfer::decode(body, encoding.as_deref());
        let message = Box::new(parse(&raw, DefaultContentType::Text, depth + 1));
        PartBody::Message { raw, message }
    } else {
        PartBody::Leaf(transfer::decode(body, encoding.as_deref()))
    };

    MimePart {
        headers,
        content_type,
        disposition,
        body,
    }
}

fn multipart(content_type: &ContentType, body: &[u8], depth: usize) -> Option<PartBody> {
    let boundary = content_type.boundary()?;
    let sections = sections(body, boundary.as_bytes())?;

    let default_type = match content_type.subtype() {
        "digest" => DefaultContentType::Message,
        _ => DefaultContentType::Text,
    };
    let parts: Vec<MimePart> = sections
        .iter()
        .map(|section| parse(section, default_type, depth + 1))
        .collect();

    let protection = match (
        content_type.subtype(),
        sections.as_slice(),
        parts.as_slice(),
    ) {
        ("signed", [signed, ..], [_, signature, ..]) => Protection::Signed {
            signed: to_crlf(signed),
            signature: signature.data().map(<[u8]>::to_vec).unwrap_or_default(),
            status: None,
        },
        ("encrypted", _, [_, payload, ..]) => Protection::Encrypted {
            payload: payload.data().map(<[u8]>::to_vec).unwrap_or_default(),
            decrypted: None,
            status: None,
        },
        _ => Protection::Plain,
    };

    Some(PartBody::Multipart(Multipart { parts, protection }))
}

fn sections<'a>(body: &'a [u8], boundary: &[u8]) -> Option<Vec<&'a [u8]>> {
    let mut sections = Vec::new();
    let mut current_start: Option<usize> = None;
    let mut has_boundary = false;
    let mut start = 0;

    while start < body.len() {
        let end = body[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(body.len(), |offset| start + offset);
        let next = (end + 1).min(body.len());
        let line = &body[start..end];
        let line = line.strip_suffix(b"\r").unwrap_or(line);

        if let Some(rest) = line
            .strip_prefix(b"--")
            .and_then(|rest| rest.strip_prefix(boundary))
        {
            let closing = rest.starts_with(b"--");
            if closing || rest.iter().all(|byte| *byte == b' ' || *byte == b'\t') {
                has_boundary = true;
                if let Some(section_start) = current_start.take() {
                    sections.push(
                        &body[section_start..end_before_line_break(body, section_start, start)],
                    );
                }
                if closing {
                    return Some(sections);
                }
                current_start = Some(next);
            }
        }

        start = next;
    }

    if let Some(section_start) = current_start {
        sections.push(&body[section_start.min(body.len())..]);
    }

    has_boundary.then_some(sections)
}

fn end_before_line_break(body: &[u8], floor: usize, line_start: usize) -> usize {
    let mut end = line_start;
    if end > floor && body[end - 1] == b'\n' {
        end -= 1;
        if end > floor && body[end - 1] == b'\r' {
            end -= 1;
        }
    }
    end
}

pub(crate) fn split(raw: &[u8]) -> (&[u8], &[u8]) {
    let mut start = 0;

    while start < raw.len() {
        let end = raw[start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(raw.len(), |offset| start + offset);
        let next = (end + 1).min(raw.len());
        let line = &raw[start..end];
        let line = line.strip_suffix(b"\r").unwrap_or(line);

        if line.is_empty() {
            return (&raw[..start], &raw[next..]);
        }

        let continues = start > 0 && (line[0] == b' ' || line[0] == b'\t');
        if !continues && field_name_end(line).is_none() {
            return (&raw[..start], &raw[start..]);
        }

        start = next;
    }

    (raw, &[])
}

pub(crate) fn to_crlf(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len() + data.len() / 32);
    let mut previous = 0u8;

    for byte in data {
        if *byte == b'\n' && previous != b'\r' {
            output.push(b'\r');
        }
        output.push(*byte);
        previous = *byte;
    }

    output
}
