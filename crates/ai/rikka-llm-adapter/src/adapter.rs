use futures_util::future::BoxFuture;
use rikka_llm::{Meter, Model, ModelInfo};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::auth::Auth;
use crate::error::Result;
use crate::listing::Listing;
use crate::service::Service;

pub type Models = BoxFuture<'static, rikka_llm::Result<Vec<ModelInfo>>>;

pub trait Adapter: Send + Sync + 'static {
    type Tokens: Serialize + DeserializeOwned + Send + Sync + 'static;

    const SERVICE: Service<Self::Tokens>;

    fn connect(&self, auth: &Auth<'_, Self::Tokens>, model: &ModelInfo) -> Result<Model>;

    fn who(&self, tokens: &Self::Tokens) -> String {
        let _ = tokens;
        String::new()
    }

    fn meter(&self, auth: &Auth<'_, Self::Tokens>) -> Option<Result<Meter>> {
        let _ = auth;
        None
    }

    fn listing(&self, auth: &Auth<'_, Self::Tokens>) -> Option<Result<Listing>> {
        let _ = auth;
        None
    }

    fn catalogue(&self) -> Option<Models> {
        None
    }
}
