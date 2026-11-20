mod markup;
mod style;

use std::fmt::{self, Write};

use markup::Markup;
use rikka_html::Escaped;

use crate::article::Article;
use crate::look::Look;

const POLICY: &str = "default-src 'none'; img-src https: http: data:; style-src 'unsafe-inline'";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Labels {
    pub reading_time: Option<String>,
    pub published: Option<String>,
}

pub fn render(article: &Article, look: &Look, labels: &Labels) -> String {
    let mut out = String::with_capacity(16 * 1024);
    document(&mut out, article, look, labels).expect("writing into a String cannot fail");
    out
}

fn document(out: &mut impl Write, article: &Article, look: &Look, labels: &Labels) -> fmt::Result {
    let ideographic = article.ideographic();
    out.write_str("<!DOCTYPE html>\n<html")?;
    if let Some(language) = article.language() {
        write!(out, " lang=\"{}\"", Escaped(language))?;
    }
    write!(out, " dir=\"{}\"", article.direction().as_str())?;
    if ideographic {
        out.write_str(" class=\"ideographic\"")?;
    }
    let scheme = if look.palette.is_dark() {
        "dark"
    } else {
        "light"
    };
    write!(
        out,
        ">\n<head>\n<meta charset=\"utf-8\">\n<meta http-equiv=\"Content-Security-Policy\" content=\"{POLICY}\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"color-scheme\" content=\"{scheme}\">\n<title>{}</title>\n<style>\n{}</style>\n</head>\n<body>\n<article>\n",
        Escaped(article.title()),
        style::sheet(look, ideographic),
    )?;
    header(out, article, labels)?;
    let mut markup = Markup::new(out, article.blocks());
    if let Some(lead) = article.lead() {
        let mut repeated = false;
        for block in article.blocks() {
            block.each_image(&mut |image| repeated |= image.source == lead.source);
        }
        if !repeated {
            markup.lead(lead)?;
        }
    }
    markup.blocks(article.blocks())?;
    out.write_str("</article>\n</body>\n</html>\n")
}

fn header(out: &mut impl Write, article: &Article, labels: &Labels) -> fmt::Result {
    out.write_str("<header>\n")?;
    if let Some(site) = article.site() {
        writeln!(out, "<p class=\"site\">{}</p>", Escaped(site))?;
    }
    writeln!(out, "<h1>{}</h1>", Escaped(article.title()))?;
    if let Some(byline) = article.byline() {
        writeln!(out, "<p class=\"byline\">{}</p>", Escaped(byline))?;
    }
    let published = labels
        .published
        .as_deref()
        .filter(|label| !label.is_empty());
    let reading = labels
        .reading_time
        .as_deref()
        .filter(|label| !label.is_empty());
    if published.is_some() || reading.is_some() {
        out.write_str("<p class=\"meta\">")?;
        if let Some(published) = published {
            out.write_str("<span><time")?;
            if let Some(date) = article.published_date() {
                write!(
                    out,
                    " datetime=\"{:04}-{:02}-{:02}\"",
                    date.year, date.month, date.day
                )?;
            }
            write!(out, ">{}</time></span>", Escaped(published))?;
        }
        if let Some(reading) = reading {
            write!(out, "<span>{}</span>", Escaped(reading))?;
        }
        out.write_str("</p>\n")?;
    }
    out.write_str("</header>\n")
}
