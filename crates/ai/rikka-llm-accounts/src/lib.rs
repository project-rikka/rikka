mod account;
mod account_id;
mod account_store;
mod chosen;
mod connected_model;
mod effort_label;
mod error;
mod listing;
mod llm;
mod login;
mod model_options;
mod reading;
mod readings;
mod roster;
mod selection;
mod verification;

pub use account::Account;
pub use account_id::AccountId;
pub use connected_model::ConnectedModel;
pub use effort_label::effort_label;
pub use error::{Error, Result};
pub use llm::{ANY_LANGUAGE, AccountsChanged, Llm};
pub use login::{Step, sign_in};
pub use model_options::{ModelOption, ModelOptions, Row};
pub use reading::Reading;
pub use readings::{Readings, UsageQuery, UsageReport};
pub use rikka_llm_providers::{
    Backend, Checkup, Credential, Endpoint, EndpointCredentials, Header, Listing, Offered, Prompt,
    SignIn, Wire,
};
pub use roster::ModelRoster;
pub use selection::Selection;
pub use verification::{open_verification, verification_label};

pub static CATALOGS: &[(&str, &str)] = rikka_i18n::catalogs!();
