mod allowance;
mod auth;
mod client;
mod hub;

use rikka_llm::{Meter, Model, ModelInfo};
use rikka_llm_adapter::{Adapter, Auth, Listing, Result, Service, SignInMethod};

pub use auth::AntigravityCredentials;

#[derive(Debug)]
pub struct AntigravityAdapter;

impl Adapter for AntigravityAdapter {
    type Tokens = AntigravityCredentials;

    const SERVICE: Service<AntigravityCredentials> = Service {
        id: "antigravity",
        label: "Antigravity",
        sign_in: SignInMethod::Browser(auth::login),
    };

    fn who(&self, tokens: &AntigravityCredentials) -> String {
        tokens.email().to_owned()
    }

    fn connect(&self, auth: &Auth<'_, AntigravityCredentials>, model: &ModelInfo) -> Result<Model> {
        client::connect(auth, model)
    }

    fn meter(&self, auth: &Auth<'_, AntigravityCredentials>) -> Option<Result<Meter>> {
        Some(client::meter(auth))
    }

    fn listing(&self, auth: &Auth<'_, AntigravityCredentials>) -> Option<Result<Listing>> {
        Some(client::listing(auth))
    }
}
