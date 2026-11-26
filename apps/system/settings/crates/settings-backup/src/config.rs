use rikka_settings::seam::Preferences;
use rikka_settings::{Key, Schema};

const DOMAIN: &str = "rikka.backup";
const REPOSITORY: &str = "repository";

#[derive(Debug)]
pub struct Config(Preferences);

impl Config {
    #[must_use]
    pub fn load() -> Config {
        Config(Preferences::open(Config::schema()))
    }

    #[must_use]
    pub fn schema() -> Schema {
        Schema::new(DOMAIN).summary("Backup").key(
            Key::string(REPOSITORY, "")
                .summary("Git repository that receives this computer's configuration"),
        )
    }

    #[must_use]
    pub fn repository(&mut self) -> String {
        self.0.refresh();
        self.0.get(REPOSITORY)
    }

    pub fn set_repository(&mut self, url: &str) -> bool {
        self.0.set(REPOSITORY, url)
    }
}
