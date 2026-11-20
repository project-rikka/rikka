use rikka_proton_pgp::{Pgp, PrivateKey};

use crate::error::Result;
use crate::mime::{
    ContentDisposition, ContentType, HeaderField, Headers, MimeMessage, MimePart, PartBody,
    decode_charset, transfer,
};

const PRESERVED_HEADERS: &[&str] = &[
    "Message-Id",
    "From",
    "To",
    "Subject",
    "Date",
    "Received",
    "Cc",
];

pub(crate) fn encrypt_for_import(pgp: &Pgp, key: &PrivateKey, literal: &[u8]) -> Result<Vec<u8>> {
    let message = MimeMessage::parse(literal);

    let mut text_parts = 0;
    if is_encryptable_per_part(message.root(), &mut text_parts) {
        let mut output = Vec::with_capacity(literal.len() * 2);
        if encrypt_part(pgp, key, message.root(), &mut output)? {
            return Ok(output);
        }
    }

    encrypt_as_pgp_mime(pgp, key, literal, message.headers())
}

fn is_encryptable_per_part(part: &MimePart, text_parts: &mut usize) -> bool {
    let headers = part.headers();
    let declared = match headers.unfolded("content-type") {
        Some(value) => match ContentType::parse(&value) {
            Some(declared) => Some(declared),
            None => return false,
        },
        None => None,
    };
    let readable_disposition = headers
        .unfolded("content-disposition")
        .is_none_or(|value| ContentDisposition::parse(&value).is_some());
    if !readable_disposition {
        return false;
    }

    let content_type = part.content_type();
    if matches!(content_type.subtype(), "encrypted" | "signed") {
        return false;
    }

    match part.body() {
        PartBody::Multipart(multipart) => multipart
            .parts()
            .iter()
            .all(|child| is_encryptable_per_part(child, text_parts)),
        _ if declared.is_some_and(|declared| declared.is_multipart()) => false,
        _ if content_type.is_text() => {
            *text_parts += 1;
            *text_parts <= 1
        }
        _ => true,
    }
}

fn encrypt_part(
    pgp: &Pgp,
    key: &PrivateKey,
    part: &MimePart,
    output: &mut Vec<u8>,
) -> Result<bool> {
    let content_type = part.content_type();

    match part.body() {
        PartBody::Multipart(multipart) => {
            let Some(boundary) = content_type.boundary() else {
                return Ok(false);
            };
            part.headers().write_to(output);
            for child in multipart.parts() {
                output.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                if !encrypt_part(pgp, key, child, output)? {
                    return Ok(false);
                }
                output.extend_from_slice(b"\r\n");
            }
            output.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        }
        _ if content_type.is_text() || content_type.kind() == "message" => {
            let mut headers = part.headers().clone();
            headers.remove("content-transfer-encoding");
            let data = part.data().unwrap_or_default();

            let body = match content_type.charset() {
                Some(charset) => {
                    let text = decode_charset(data, Some(charset));
                    let mut utf8_type = content_type.clone();
                    utf8_type.set_parameter("charset", "utf-8");
                    headers.set("Content-Type", &utf8_type.header_value());
                    text.into_bytes()
                }
                None => data.to_vec(),
            };

            headers.write_to(output);
            let armored = pgp.encrypt().to(key.public_key()).armored_message(&body)?;
            output.extend_from_slice(armored.as_bytes());
        }
        _ => {
            let mut headers = part.headers().clone();
            headers.set("Content-Transfer-Encoding", "base64");
            headers.write_to(output);

            let encrypted = pgp
                .encrypt()
                .to(key.public_key())
                .message(part.data().unwrap_or_default())?;
            output.extend_from_slice(&transfer::base64_lines(&encrypted));
        }
    }

    Ok(true)
}

fn encrypt_as_pgp_mime(
    pgp: &Pgp,
    key: &PrivateKey,
    literal: &[u8],
    original: &Headers,
) -> Result<Vec<u8>> {
    let armored = pgp
        .encrypt()
        .to(key.public_key())
        .signed_by(key.clone())
        .armored_message(literal)?;

    let boundary = rikka_proton_api::multipart_boundary();

    let mut headers = Headers::default();
    headers.push(HeaderField::new("Mime-Version", "1.0"));
    headers.push(HeaderField::new(
        "Content-Type",
        &format!(
            "multipart/encrypted; boundary=\"{boundary}\"; protocol=\"application/pgp-encrypted\""
        ),
    ));
    for name in PRESERVED_HEADERS {
        if let Some(field) = original.field(name) {
            headers.push(field.clone());
        }
    }

    let mut output = Vec::with_capacity(armored.len() + 1024);
    headers.write_to(&mut output);
    output.extend_from_slice(
        format!(
            "--{boundary}\r\n\
             Content-Description: PGP/MIME version identification\r\n\
             Content-Type: application/pgp-encrypted\r\n\
             \r\n\
             Version: 1\r\n\
             --{boundary}\r\n\
             Content-Description: OpenPGP encrypted message\r\n\
             Content-Disposition: inline; filename=encrypted.asc\r\n\
             Content-Type: application/octet-stream; name=encrypted.asc\r\n\
             \r\n"
        )
        .as_bytes(),
    );
    output.extend_from_slice(armored.as_bytes());
    output.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    Ok(output)
}
