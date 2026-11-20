use serde::Serialize;

use crate::account::Account;
use crate::address::Address;
use crate::error::Result;

const ADDRESSES_PATH: &str = "core/v4/addresses";
const SETUP_PATH: &str = "core/v4/addresses/setup";

#[must_use]
#[derive(Debug)]
pub struct AddressCreation {
    account: Account,
    local_part: Option<String>,
    domain: String,
    display_name: Option<String>,
    signature: Option<String>,
}

impl AddressCreation {
    pub fn local_part(mut self, local_part: impl Into<String>) -> Self {
        self.local_part = Some(local_part.into());
        self
    }

    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    pub fn signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    pub async fn execute(self) -> Result<Address> {
        let path = match self.local_part {
            Some(_) => ADDRESSES_PATH,
            None => SETUP_PATH,
        };
        let request = CreationRequest {
            local_part: self.local_part.as_deref(),
            domain: &self.domain,
            display_name: self.display_name.as_deref(),
            signature: self.signature.as_deref(),
        };
        let response: Address = self
            .account
            .api()
            .post(path)
            .json(&request)
            .send_field("Address")
            .await?;

        Ok(response)
    }

    pub(crate) fn new(account: Account, domain: String) -> AddressCreation {
        AddressCreation {
            account,
            local_part: None,
            domain,
            display_name: None,
            signature: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct CreationRequest<'a> {
    #[serde(rename = "Local", skip_serializing_if = "Option::is_none")]
    local_part: Option<&'a str>,
    #[serde(rename = "Domain")]
    domain: &'a str,
    #[serde(rename = "DisplayName", skip_serializing_if = "Option::is_none")]
    display_name: Option<&'a str>,
    #[serde(rename = "Signature", skip_serializing_if = "Option::is_none")]
    signature: Option<&'a str>,
}
