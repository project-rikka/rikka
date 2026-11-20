use rikka_llm_accounts::ConnectedModel;
use rikka_types::CancelToken;

#[derive(Clone, Debug)]
pub(crate) struct RunContext {
    pub(crate) hired: ConnectedModel,
    pub(crate) key: String,
    pub(crate) cancel: CancelToken,
}
