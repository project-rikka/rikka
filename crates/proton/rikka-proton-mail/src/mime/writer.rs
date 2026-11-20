use std::fmt::{self, Write};

use crate::mime::html_text::html_to_text;
use crate::mime::parameters::{ContentType, HeaderParameter};
use crate::mime::transfer;

const OCTET_STREAM: &str = "application/octet-stream";

#[derive(Debug, Clone, Copy)]
pub(crate) struct OutgoingFile<'a> {
    pub(crate) name: &'a str,
    pub(crate) mime_type: &'a str,
    pub(crate) content_id: Option<&'a str>,
    pub(crate) data: &'a [u8],
}

#[derive(Debug, Clone)]
pub(crate) struct AttachmentFile {
    pub(crate) name: String,
    pub(crate) mime_type: String,
    pub(crate) content_id: Option<String>,
    pub(crate) data: Vec<u8>,
}

impl AttachmentFile {
    pub(crate) fn as_outgoing(&self) -> OutgoingFile<'_> {
        OutgoingFile {
            name: &self.name,
            mime_type: &self.mime_type,
            content_id: self.content_id.as_deref(),
            data: &self.data,
        }
    }
}

impl OutgoingFile<'_> {
    fn is_embedded_in(&self, html_body: &str) -> bool {
        self.content_id
            .is_some_and(|id| html_body.contains(&format!("cid:{}", bare_id(id))))
    }
}

pub(crate) fn mime_body(body: &str, html: bool, files: &[OutgoingFile<'_>]) -> String {
    MimeBody { body, html, files }.to_string()
}

struct MimeBody<'a> {
    body: &'a str,
    html: bool,
    files: &'a [OutgoingFile<'a>],
}

impl MimeBody<'_> {
    fn is_inline(&self, file: &OutgoingFile<'_>) -> bool {
        self.html && file.is_embedded_in(self.body)
    }

    fn write_text(&self, out: &mut impl Write) -> fmt::Result {
        let plain = match self.html {
            true => html_to_text(self.body),
            false => self.body.to_owned(),
        };
        out.write_str(
            "Content-Type: text/plain; charset=utf-8\r\n\
             Content-Transfer-Encoding: quoted-printable\r\n\r\n",
        )?;
        transfer::write_quoted_printable(out, &plain)
    }

    fn write_html(&self, out: &mut impl Write) -> fmt::Result {
        out.write_str(
            "Content-Type: text/html; charset=utf-8\r\n\
             Content-Transfer-Encoding: base64\r\n\r\n",
        )?;
        write_base64_lines(out, self.body.as_bytes())
    }

    fn write_related(&self, out: &mut impl Write) -> fmt::Result {
        let mut related = Multipart::begin(out, "related")?;
        related.part(out, |out| self.write_html(out))?;
        for file in self.files.iter().filter(|file| self.is_inline(file)) {
            related.part(out, |out| write_file_entity(out, file, "inline"))?;
        }
        related.end(out)
    }

    fn write_alternative(&self, out: &mut impl Write) -> fmt::Result {
        let mut alternative = Multipart::begin(out, "alternative")?;
        alternative.part(out, |out| self.write_text(out))?;
        alternative.part(out, |out| self.write_related(out))?;
        alternative.end(out)
    }
}

impl fmt::Display for MimeBody<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut mixed = Multipart::begin(out, "mixed")?;
        mixed.part(out, |out| match self.html {
            true => self.write_alternative(out),
            false => self.write_text(out),
        })?;
        for file in self.files.iter().filter(|file| !self.is_inline(file)) {
            mixed.part(out, |out| write_file_entity(out, file, "attachment"))?;
        }
        mixed.end(out)?;
        out.write_str("\r\n")
    }
}

struct Multipart {
    boundary: String,
}

impl Multipart {
    fn begin(out: &mut impl Write, subtype: &str) -> Result<Multipart, fmt::Error> {
        let boundary = rikka_proton_api::multipart_boundary();
        write!(
            out,
            "Content-Type: multipart/{subtype}; boundary=\"{boundary}\"\r\n\r\n"
        )?;
        Ok(Multipart { boundary })
    }

    fn part<W: Write>(
        &mut self,
        out: &mut W,
        write_entity: impl FnOnce(&mut W) -> fmt::Result,
    ) -> fmt::Result {
        write!(out, "--{}\r\n", self.boundary)?;
        write_entity(out)?;
        out.write_str("\r\n")
    }

    fn end(self, out: &mut impl Write) -> fmt::Result {
        write!(out, "--{}--", self.boundary)
    }
}

fn write_file_entity(
    out: &mut impl Write,
    file: &OutgoingFile<'_>,
    disposition: &str,
) -> fmt::Result {
    let essence = ContentType::parse(file.mime_type)
        .map_or_else(|| OCTET_STREAM.to_owned(), |parsed| parsed.essence());
    let filename = HeaderParameter::new("filename", file.name);
    let name = HeaderParameter::new("name", file.name);

    write!(
        out,
        "Content-Type: {essence}{filename}{name}\r\n\
         Content-Transfer-Encoding: base64\r\n\
         Content-Disposition: {disposition}{filename}{name}\r\n"
    )?;
    if let Some(id) = file.content_id {
        write!(out, "Content-ID: <{}>\r\n", bare_id(id))?;
    }
    out.write_str("\r\n")?;
    write_base64_lines(out, file.data)
}

fn write_base64_lines(out: &mut impl Write, data: &[u8]) -> fmt::Result {
    let wrapped = transfer::base64_lines(data);
    out.write_str(String::from_utf8_lossy(&wrapped).trim_end_matches("\r\n"))
}

fn bare_id(id: &str) -> &str {
    id.trim().trim_start_matches('<').trim_end_matches('>')
}
