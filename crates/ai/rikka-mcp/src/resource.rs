use crate::prompt::worded;
use crate::protocol::{Resource as Offered, ResourceTemplate as OfferedTemplate};

const BETWEEN: char = ':';

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    server: String,
    mention: String,
    uri: String,
    name: String,
    title: Option<String>,
    description: Option<String>,
    mime: Option<String>,
    size: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceTemplate {
    server: String,
    mention: String,
    fixed: usize,
    uri_template: String,
    name: String,
    title: Option<String>,
    description: Option<String>,
    mime: Option<String>,
}

impl Resource {
    #[must_use]
    pub fn server(&self) -> &str {
        &self.server
    }

    #[must_use]
    pub fn mention(&self) -> &str {
        &self.mention
    }

    #[must_use]
    pub fn uri(&self) -> &str {
        &self.uri
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    #[must_use]
    pub fn mime(&self) -> Option<&str> {
        self.mime.as_deref()
    }

    #[must_use]
    pub fn size(&self) -> Option<u64> {
        self.size
    }
}

impl ResourceTemplate {
    #[must_use]
    pub fn server(&self) -> &str {
        &self.server
    }

    #[must_use]
    pub fn uri_template(&self) -> &str {
        &self.uri_template
    }

    #[must_use]
    pub fn mention_template(&self) -> &str {
        &self.mention
    }

    #[must_use]
    pub fn mention_prefix(&self) -> &str {
        &self.mention[..self.fixed]
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    #[must_use]
    pub fn mime(&self) -> Option<&str> {
        self.mime.as_deref()
    }
}

pub(crate) fn listed(server: &str, label: &str, offered: Vec<Offered>) -> Vec<Resource> {
    offered
        .into_iter()
        .map(|resource| Resource {
            server: server.to_owned(),
            mention: mentioned(label, &resource.uri),
            name: resource.name,
            title: worded(resource.title),
            description: worded(resource.description),
            mime: resource.mime_type,
            size: resource.size,
            uri: resource.uri,
        })
        .collect()
}

pub(crate) fn templated(
    server: &str,
    label: &str,
    offered: Vec<OfferedTemplate>,
) -> Vec<ResourceTemplate> {
    offered
        .into_iter()
        .map(|template| {
            let fixed = template
                .uri_template
                .find('{')
                .unwrap_or(template.uri_template.len());
            ResourceTemplate {
                server: server.to_owned(),
                mention: mentioned(label, &template.uri_template),
                fixed: label.len() + BETWEEN.len_utf8() + fixed,
                uri_template: template.uri_template,
                name: template.name,
                title: worded(template.title),
                description: worded(template.description),
                mime: template.mime_type,
            }
        })
        .collect()
}

pub(crate) fn split(mention: &str) -> Option<(&str, &str)> {
    mention
        .split_once(BETWEEN)
        .filter(|(label, uri)| !label.is_empty() && !uri.is_empty())
}

fn mentioned(label: &str, uri: &str) -> String {
    format!("{label}{BETWEEN}{uri}")
}
