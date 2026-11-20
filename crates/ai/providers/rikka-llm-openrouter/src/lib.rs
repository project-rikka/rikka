mod auth;
mod catalogue;
mod client;

use futures_util::FutureExt;
use futures_util::future::BoxFuture;
use rikka_llm::{Model, ModelInfo};
use rikka_llm_adapter::{
    Adapter, Api, Auth, BrowserLogin, Grant, NoTokens, Result, Service, SignInMethod,
};

use crate::auth::OpenRouterLogin;

pub use client::{OpenRouter, OpenRouterApi};

const APP_NAME: &str = "Rikka";
const APP_URL: &str = "https://github.com/project-rikka/rikka";

#[derive(Debug)]
pub struct OpenRouterAdapter;

impl Adapter for OpenRouterAdapter {
    type Tokens = NoTokens;

    const SERVICE: Service<NoTokens> = Service {
        id: "openrouter",
        label: "OpenRouter",
        sign_in: SignInMethod::KeyOrBrowser {
            variable: OpenRouterApi::VARIABLE,
            browser: login,
        },
    };

    fn connect(&self, auth: &Auth<'_, NoTokens>, model: &ModelInfo) -> Result<Model> {
        Ok(OpenRouter::new(auth.key()?)
            .with(|api| api.app(APP_NAME, APP_URL))
            .model(model.id())
            .build()?)
    }

    fn catalogue(&self) -> Option<BoxFuture<'static, rikka_llm::Result<Vec<ModelInfo>>>> {
        Some(catalogue::fetch().boxed())
    }
}

fn login() -> BrowserLogin<NoTokens> {
    BrowserLogin::start(async {
        let login = OpenRouterLogin::start().await?;
        let url = login.url().to_owned();
        Ok((url, async move {
            let tokens = login.finish().await?;
            Ok(Grant::<NoTokens>::Key(tokens.key().to_owned()))
        }))
    })
}
