use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::account::Account;
use crate::error::Result;
use crate::reauthentication::Reauthentication;

const PERSIST_PASSWORD_SCOPE: &str = "PersistPasswordScope";
const SERVER_PROOF: &str = "ServerProof";

#[derive(Debug, Clone, Copy)]
pub(crate) enum Verb {
    Put,
    Post,
}

#[derive(Debug)]
pub(crate) struct ReauthenticatedRequest {
    account: Account,
    verb: Verb,
    path: String,
    body: Map<String, Value>,
    pub(crate) reauthentication: Reauthentication,
    persist_password_scope: bool,
}

impl ReauthenticatedRequest {
    pub(crate) fn new(account: Account, verb: Verb, path: impl Into<String>) -> Self {
        ReauthenticatedRequest {
            account,
            verb,
            path: path.into(),
            body: Map::new(),
            reauthentication: Reauthentication::default(),
            persist_password_scope: false,
        }
    }

    pub(crate) fn keep_password_scope(&mut self) {
        self.persist_password_scope = true;
    }

    pub(crate) fn account(&self) -> &Account {
        &self.account
    }

    pub(crate) fn with_field(mut self, name: &str, value: impl Into<Value>) -> Self {
        self.body.insert(name.to_owned(), value.into());
        self
    }

    pub(crate) fn extend(&mut self, fields: &impl Serialize) -> Result<()> {
        merge_fields(&mut self.body, fields)
    }

    pub(crate) async fn send<T: DeserializeOwned>(self) -> Result<T> {
        let path = self.path.clone();
        decoded(&path, self.answer().await?)
    }

    pub(crate) async fn send_field_or_default<T: DeserializeOwned + Default>(
        self,
        name: &str,
    ) -> Result<T> {
        let path = self.path.clone();
        match self.answer().await? {
            Value::Object(mut fields) => match fields.remove(name) {
                Some(field) => decoded(&path, field),
                None => Ok(T::default()),
            },
            _ => Ok(T::default()),
        }
    }

    async fn answer(self) -> Result<Value> {
        let proof = self.reauthentication.prove(&self.account).await?;
        let mut body = self.body;

        if let Some(proof) = &proof {
            merge_fields(&mut body, proof.request())?;
        }
        if self.persist_password_scope {
            body.insert(PERSIST_PASSWORD_SCOPE.to_owned(), Value::Bool(true));
        }

        let api = self.account.api();
        let request = match self.verb {
            Verb::Put => api.put(&self.path),
            Verb::Post => api.post(&self.path),
        };
        let response: Value = request.json(&body).send().await?;

        if let Some(proof) = &proof {
            proof.verify(response.get(SERVER_PROOF).and_then(Value::as_str))?;
        }

        Ok(response)
    }
}

fn decoded<T: DeserializeOwned>(path: &str, value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|source| {
        rikka_proton_api::Error::Decode {
            request: path.to_owned(),
            source,
        }
        .into()
    })
}

fn merge_fields(body: &mut Map<String, Value>, fields: &impl Serialize) -> Result<()> {
    if let Value::Object(fields) =
        serde_json::to_value(fields).map_err(rikka_proton_api::Error::Encode)?
    {
        body.extend(fields);
    }
    Ok(())
}
