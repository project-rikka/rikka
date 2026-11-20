use rikka_net::Url;
use serde_json::Value;

use crate::client::endpoint;
use crate::error::{Error, Result, text, texts};
use crate::http;
use crate::well_known;

const WELL_KNOWN: &str = "oauth-protected-resource";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedResource {
    resource: Url,
    authorization_servers: Vec<Url>,
    scopes_supported: Vec<String>,
}

impl ProtectedResource {
    pub async fn discover(resource: &str) -> Result<Option<ProtectedResource>> {
        let resource = endpoint(resource)?;
        let client = http::client()?;
        let locations = well_known::candidates([
            well_known::inserted(&resource, WELL_KNOWN),
            well_known::rooted(&resource, WELL_KNOWN),
        ]);
        for location in locations {
            if let Some(document) = http::found(http::document(&client, location)).await? {
                return ProtectedResource::from_document(&document, &resource).map(Some);
            }
        }
        Ok(None)
    }

    pub async fn fetch(metadata: &str, resource: &str) -> Result<Option<ProtectedResource>> {
        let resource = endpoint(resource)?;
        let request = http::document(&http::client()?, endpoint(metadata)?);
        match http::found(request).await? {
            Some(document) => ProtectedResource::from_document(&document, &resource).map(Some),
            None => Ok(None),
        }
    }

    fn from_document(document: &Value, requested: &Url) -> Result<ProtectedResource> {
        let named = match text(document, "resource") {
            named if named.is_empty() => return Err(Error::MissingField("resource")),
            named => named,
        };
        let resource = endpoint(&named)?;
        if resource.fragment().is_some() || !covers(&resource, requested) {
            return Err(Error::ResourceMismatch {
                expected: requested.to_string(),
                found: named,
            });
        }

        Ok(ProtectedResource {
            resource,
            authorization_servers: texts(document, "authorization_servers")
                .unwrap_or_default()
                .iter()
                .filter_map(|server| endpoint(server).ok())
                .collect(),
            scopes_supported: texts(document, "scopes_supported").unwrap_or_default(),
        })
    }

    #[must_use]
    pub fn resource(&self) -> &Url {
        &self.resource
    }

    #[must_use]
    pub fn authorization_servers(&self) -> &[Url] {
        &self.authorization_servers
    }

    #[must_use]
    pub fn scopes_supported(&self) -> &[String] {
        &self.scopes_supported
    }
}

fn covers(resource: &Url, requested: &Url) -> bool {
    if resource.scheme() != requested.scheme()
        || resource.host_str() != requested.host_str()
        || resource.port_or_known_default() != requested.port_or_known_default()
    {
        return false;
    }

    let within = resource.path().trim_end_matches('/');
    match requested.path().strip_prefix(within) {
        Some(below) => below.is_empty() || below.starts_with('/'),
        None => false,
    }
}
