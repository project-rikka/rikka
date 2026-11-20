use serde::Serialize;

use crate::account::Account;
use crate::address_creation::AddressCreation;
use crate::error::Result;
use crate::identifier::AddressId;

const ADDRESSES_PATH: &str = "core/v4/addresses";

impl Account {
    pub fn create_address(&self, domain: impl Into<String>) -> AddressCreation {
        AddressCreation::new(self.clone(), domain.into())
    }

    pub fn update_address(&self, id: &AddressId) -> AddressUpdate {
        AddressUpdate {
            account: self.clone(),
            id: id.clone(),
            display_name: None,
            signature: None,
        }
    }

    pub async fn reorder_addresses(&self, order: &[AddressId]) -> Result<()> {
        self.api()
            .put(format!("{ADDRESSES_PATH}/order"))
            .json(&OrderRequest { ids: order })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn enable_address(&self, id: &AddressId) -> Result<()> {
        self.address_action(id, "enable").await
    }

    pub async fn disable_address(&self, id: &AddressId) -> Result<()> {
        self.address_action(id, "disable").await
    }

    pub async fn delete_address(&self, id: &AddressId) -> Result<()> {
        self.address_action(id, "delete").await
    }

    async fn address_action(&self, id: &AddressId, action: &str) -> Result<()> {
        self.api()
            .put(format!("{ADDRESSES_PATH}/{id}/{action}"))
            .send_unit()
            .await?;

        Ok(())
    }
}

#[must_use]
#[derive(Debug)]
pub struct AddressUpdate {
    account: Account,
    id: AddressId,
    display_name: Option<String>,
    signature: Option<String>,
}

impl AddressUpdate {
    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    pub fn signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }

    pub async fn execute(self) -> Result<()> {
        let request = UpdateRequest {
            display_name: self.display_name.as_deref(),
            signature: self.signature.as_deref(),
        };

        self.account
            .api()
            .put(format!("{ADDRESSES_PATH}/{}", self.id))
            .json(&request)
            .send_unit()
            .await?;

        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct OrderRequest<'a> {
    #[serde(rename = "AddressIDs")]
    ids: &'a [AddressId],
}

#[derive(Debug, Serialize)]
struct UpdateRequest<'a> {
    #[serde(rename = "DisplayName", skip_serializing_if = "Option::is_none")]
    display_name: Option<&'a str>,
    #[serde(rename = "Signature", skip_serializing_if = "Option::is_none")]
    signature: Option<&'a str>,
}
