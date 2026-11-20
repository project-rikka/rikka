mod allowance;
mod auth;
mod betas;
mod client;

use rikka_llm::{Meter, Model, ModelInfo};
use rikka_llm_adapter::{Adapter, Auth, Listing, Result, Service, SignInMethod, account_label};

pub use auth::ClaudeCodeCredentials;

#[derive(Debug)]
pub struct ClaudeCodeAdapter;

impl Adapter for ClaudeCodeAdapter {
    type Tokens = ClaudeCodeCredentials;

    const SERVICE: Service<ClaudeCodeCredentials> = Service {
        id: "claude-code",
        label: "Claude Code",
        sign_in: SignInMethod::Browser(auth::login),
    };

    fn who(&self, tokens: &ClaudeCodeCredentials) -> String {
        account_label(tokens.email(), tokens.organization_name())
    }

    fn connect(&self, auth: &Auth<'_, ClaudeCodeCredentials>, model: &ModelInfo) -> Result<Model> {
        client::connect(auth, model)
    }

    fn meter(&self, auth: &Auth<'_, ClaudeCodeCredentials>) -> Option<Result<Meter>> {
        Some(client::meter(auth))
    }

    fn listing(&self, auth: &Auth<'_, ClaudeCodeCredentials>) -> Option<Result<Listing>> {
        Some(client::listing(auth))
    }
}
