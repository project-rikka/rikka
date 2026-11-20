use std::sync::Arc;

use futures_util::future::BoxFuture;
use rikka_llm::{Meter, Model, ModelInfo};
use serde::Deserialize;
use serde_json::Value;

use crate::adapter::{Adapter, Models};
use crate::auth::Auth;
use crate::error::Result;
use crate::listing::Listing;
use crate::login::Login;
use crate::renewing::SharedTokens;
use crate::sign_in::SignIn;

pub struct Access<'a> {
    key: String,
    tokens: Option<&'a Value>,
    shared: Option<Arc<SharedTokens>>,
}

impl<'a> Access<'a> {
    #[must_use]
    pub fn new(
        key: String,
        tokens: Option<&'a Value>,
        shared: Option<Arc<SharedTokens>>,
    ) -> Access<'a> {
        Access {
            key,
            tokens,
            shared,
        }
    }

    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn tokens(&self) -> Option<&'a Value> {
        self.tokens
    }

    pub(crate) fn shared(&self) -> Option<Arc<SharedTokens>> {
        self.shared.clone()
    }
}

impl std::fmt::Debug for Access<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Access").finish_non_exhaustive()
    }
}

pub trait DynAdapter: Send + Sync + 'static {
    fn id(&self) -> &'static str;

    fn label(&self) -> &'static str;

    fn sign_in(&self) -> SignIn;

    fn accepts(&self, tokens: &Value) -> bool;

    fn who(&self, tokens: &Value) -> String;

    fn connect(&self, access: &Access<'_>, model: &ModelInfo) -> Result<Model>;

    fn meter(&self, access: &Access<'_>) -> Option<Result<Meter>>;

    fn listing(&self, access: &Access<'_>) -> Option<Result<Models>>;

    fn login(&self) -> Option<BoxFuture<'static, rikka_llm::Result<Login<Value>>>>;

    fn catalogue(&self) -> Option<Models>;
}

impl<A: Adapter> DynAdapter for A {
    fn id(&self) -> &'static str {
        A::SERVICE.id
    }

    fn label(&self) -> &'static str {
        A::SERVICE.label
    }

    fn sign_in(&self) -> SignIn {
        A::SERVICE.sign_in()
    }

    fn accepts(&self, tokens: &Value) -> bool {
        A::Tokens::deserialize(tokens).is_ok()
    }

    fn who(&self, tokens: &Value) -> String {
        A::Tokens::deserialize(tokens)
            .map(|tokens| Adapter::who(self, &tokens))
            .unwrap_or_default()
    }

    fn connect(&self, access: &Access<'_>, model: &ModelInfo) -> Result<Model> {
        Adapter::connect(self, &Auth::new(access), model)
    }

    fn meter(&self, access: &Access<'_>) -> Option<Result<Meter>> {
        Adapter::meter(self, &Auth::new(access))
    }

    fn listing(&self, access: &Access<'_>) -> Option<Result<Models>> {
        Adapter::listing(self, &Auth::new(access)).map(|listing| listing.map(Listing::described))
    }

    fn login(&self) -> Option<BoxFuture<'static, rikka_llm::Result<Login<Value>>>> {
        A::SERVICE.sign_in.start()
    }

    fn catalogue(&self) -> Option<Models> {
        Adapter::catalogue(self)
    }
}
