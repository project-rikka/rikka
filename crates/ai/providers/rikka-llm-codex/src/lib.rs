mod allowance;
mod auth;
mod client;

use rikka_llm::{Meter, Model, ModelInfo};
use rikka_llm_adapter::{Adapter, Auth, Listing, Result, Service, SignInMethod, account_label};

pub use auth::CodexCredentials;

#[derive(Debug)]
pub struct CodexAdapter;

impl Adapter for CodexAdapter {
    type Tokens = CodexCredentials;

    const SERVICE: Service<CodexCredentials> = Service {
        id: "codex",
        label: "Codex",
        sign_in: SignInMethod::Browser(auth::login),
    };

    fn who(&self, tokens: &CodexCredentials) -> String {
        account_label(tokens.email(), tokens.plan())
    }

    fn connect(&self, auth: &Auth<'_, CodexCredentials>, model: &ModelInfo) -> Result<Model> {
        client::connect(auth, model.id())
    }

    fn meter(&self, auth: &Auth<'_, CodexCredentials>) -> Option<Result<Meter>> {
        Some(client::meter(auth))
    }

    fn listing(&self, auth: &Auth<'_, CodexCredentials>) -> Option<Result<Listing>> {
        Some(client::listing(auth))
    }
}
