use std::fmt;

use rikka_xml::Element;

const BOOKMARK_NS: &str = "http://www.freedesktop.org/standards/desktop-bookmarks";
const MIME_NS: &str = "http://www.freedesktop.org/standards/shared-mime-info";
const FREEDESKTOP_OWNER: &str = "http://freedesktop.org";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Xbel {
    pub(crate) bookmarks: Vec<Bookmark>,
    namespaces: Vec<(Option<String>, String)>,
    pub(crate) preserved: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Bookmark {
    pub(crate) href: String,
    pub(crate) added: String,
    pub(crate) modified: String,
    pub(crate) visited: String,
    pub(crate) title: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) mime_type: Option<String>,
    pub(crate) groups: Vec<String>,
    pub(crate) applications: Vec<Registration>,
    pub(crate) private: bool,
    pub(crate) icon: Option<Icon>,
    pub(crate) unknown: Vec<String>,
    pub(crate) foreign: Vec<String>,
    pub(crate) preserved: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Registration {
    pub(crate) name: String,
    pub(crate) exec: String,
    pub(crate) modified: String,
    pub(crate) count: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Icon {
    pub(crate) href: String,
    pub(crate) mime_type: Option<String>,
}

impl Xbel {
    pub(crate) fn parse(text: &str) -> Result<Xbel, rikka_xml::Error> {
        let root = rikka_xml::parse(text)?;
        let namespaces = root
            .namespaces()
            .filter(|(prefix, _)| !matches!(prefix, Some("bookmark" | "mime")))
            .map(|(prefix, uri)| (prefix.map(str::to_owned), uri.to_owned()))
            .collect();
        let mut xbel = Xbel {
            namespaces,
            ..Xbel::default()
        };
        for child in root.children() {
            match is(child, None, "bookmark") {
                true => xbel.bookmarks.extend(Bookmark::read(child, text)),
                false => xbel.preserved.push(raw(child, text)),
            }
        }
        Ok(xbel)
    }
}

impl fmt::Display for Xbel {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(out, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
        write!(
            out,
            "<xbel version=\"1.0\"\n      xmlns:bookmark=\"{BOOKMARK_NS}\"\n      xmlns:mime=\"{MIME_NS}\""
        )?;
        for (prefix, uri) in &self.namespaces {
            match prefix {
                Some(prefix) => write!(out, "\n      xmlns:{prefix}=\"{}\"", escape(uri))?,
                None => write!(out, "\n      xmlns=\"{}\"", escape(uri))?,
            }
        }
        out.write_str("\n>\n")?;
        for bookmark in &self.bookmarks {
            bookmark.write(out)?;
        }
        for child in &self.preserved {
            writeln!(out, "  {child}")?;
        }
        out.write_str("</xbel>\n")
    }
}

impl Bookmark {
    fn read(node: &Element<'_>, text: &str) -> Option<Bookmark> {
        let mut bookmark = Bookmark {
            href: node.attribute("href")?.to_owned(),
            added: node.attribute("added").unwrap_or_default().to_owned(),
            modified: node.attribute("modified").unwrap_or_default().to_owned(),
            visited: node.attribute("visited").unwrap_or_default().to_owned(),
            ..Bookmark::default()
        };
        for child in node.children() {
            if is(child, None, "title") {
                bookmark.title = Some(text_of(child));
            } else if is(child, None, "desc") {
                bookmark.description = Some(text_of(child));
            } else if is(child, None, "info") {
                for metadata in child.children() {
                    match is(metadata, None, "metadata")
                        && metadata.attribute("owner") == Some(FREEDESKTOP_OWNER)
                    {
                        true => bookmark.read_metadata(metadata, text),
                        false => bookmark.foreign.push(raw(metadata, text)),
                    }
                }
            } else {
                bookmark.preserved.push(raw(child, text));
            }
        }
        Some(bookmark)
    }

    fn read_metadata(&mut self, metadata: &Element<'_>, text: &str) {
        for entry in metadata.children() {
            if is(entry, Some(MIME_NS), "mime-type") {
                self.mime_type = entry.attribute("type").map(str::to_owned);
            } else if is(entry, Some(BOOKMARK_NS), "groups") {
                self.groups = entry
                    .children()
                    .filter(|node| is(node, Some(BOOKMARK_NS), "group"))
                    .map(text_of)
                    .collect();
            } else if is(entry, Some(BOOKMARK_NS), "applications") {
                self.applications = entry
                    .children()
                    .filter(|node| is(node, Some(BOOKMARK_NS), "application"))
                    .filter_map(Registration::read)
                    .collect();
            } else if is(entry, Some(BOOKMARK_NS), "private") {
                self.private = true;
            } else if is(entry, Some(BOOKMARK_NS), "icon") {
                self.icon = entry.attribute("href").map(|href| Icon {
                    href: href.to_owned(),
                    mime_type: entry.attribute("type").map(str::to_owned),
                });
            } else {
                self.unknown.push(raw(entry, text));
            }
        }
    }

    fn write(&self, out: &mut impl fmt::Write) -> fmt::Result {
        writeln!(
            out,
            "  <bookmark href=\"{}\" added=\"{}\" modified=\"{}\" visited=\"{}\">",
            escape(&self.href),
            escape(&self.added),
            escape(&self.modified),
            escape(&self.visited)
        )?;
        if let Some(title) = &self.title {
            writeln!(out, "    <title>{}</title>", escape(title))?;
        }
        if let Some(description) = &self.description {
            writeln!(out, "    <desc>{}</desc>", escape(description))?;
        }
        out.write_str("    <info>\n")?;
        writeln!(out, "      <metadata owner=\"{FREEDESKTOP_OWNER}\">")?;
        if let Some(mime_type) = &self.mime_type {
            writeln!(
                out,
                "        <mime:mime-type type=\"{}\"/>",
                escape(mime_type)
            )?;
        }
        if !self.groups.is_empty() {
            out.write_str("        <bookmark:groups>\n")?;
            for group in &self.groups {
                writeln!(
                    out,
                    "          <bookmark:group>{}</bookmark:group>",
                    escape(group)
                )?;
            }
            out.write_str("        </bookmark:groups>\n")?;
        }
        if !self.applications.is_empty() {
            out.write_str("        <bookmark:applications>\n")?;
            for application in &self.applications {
                writeln!(
                    out,
                    "          <bookmark:application name=\"{}\" exec=\"{}\" modified=\"{}\" count=\"{}\"/>",
                    escape(&application.name),
                    escape(&application.exec),
                    escape(&application.modified),
                    application.count
                )?;
            }
            out.write_str("        </bookmark:applications>\n")?;
        }
        if self.private {
            out.write_str("        <bookmark:private/>\n")?;
        }
        if let Some(icon) = &self.icon {
            write!(
                out,
                "        <bookmark:icon href=\"{}\"",
                escape(&icon.href)
            )?;
            if let Some(mime_type) = &icon.mime_type {
                write!(out, " type=\"{}\"", escape(mime_type))?;
            }
            out.write_str("/>\n")?;
        }
        for entry in &self.unknown {
            writeln!(out, "        {entry}")?;
        }
        out.write_str("      </metadata>\n")?;
        for metadata in &self.foreign {
            writeln!(out, "      {metadata}")?;
        }
        out.write_str("    </info>\n")?;
        for child in &self.preserved {
            writeln!(out, "    {child}")?;
        }
        out.write_str("  </bookmark>\n")
    }
}

impl Registration {
    fn read(node: &Element<'_>) -> Option<Registration> {
        Some(Registration {
            name: node.attribute("name")?.to_owned(),
            exec: node.attribute("exec").unwrap_or_default().to_owned(),
            modified: node
                .attribute("modified")
                .or_else(|| node.attribute("timestamp"))
                .unwrap_or_default()
                .to_owned(),
            count: node
                .attribute("count")
                .and_then(|count| count.parse().ok())
                .unwrap_or(1),
        })
    }
}

fn is(node: &Element<'_>, namespace: Option<&str>, name: &str) -> bool {
    node.name() == name && node.namespace() == namespace
}

fn raw(node: &Element<'_>, text: &str) -> String {
    text[node.range()].to_owned()
}

fn text_of(node: &Element<'_>) -> String {
    node.text().into_owned()
}

fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}
