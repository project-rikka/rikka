use ade_config::Config;
use rikka_llm_accounts::Llm;

#[derive(Debug)]
pub(crate) struct Setup {
    pub(crate) llm: Llm,
    pub(crate) config: Config,
}

impl Setup {
    pub(crate) fn load() -> Setup {
        Setup {
            llm: Llm::load(),
            config: Config::load(),
        }
    }
}
