use crate::frame_commit::Config;

const CAPACITY: usize = 8;

#[derive(Debug, Default)]
pub(crate) struct Validated {
    configs: [Option<Config>; CAPACITY],
}

impl Validated {
    pub(crate) fn lookup(&mut self, config: &Config) -> bool {
        let Some(index) = self
            .configs
            .iter()
            .position(|cached| cached.as_ref() == Some(config))
        else {
            return false;
        };
        self.configs[..=index].rotate_right(1);
        true
    }

    pub(crate) fn insert(&mut self, config: Config) {
        if self.lookup(&config) {
            return;
        }
        self.configs.rotate_right(1);
        self.configs[0] = Some(config);
    }

    pub(crate) fn clear(&mut self) {
        self.configs = [None; CAPACITY];
    }
}
