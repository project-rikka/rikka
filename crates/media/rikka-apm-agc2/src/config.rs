#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub fixed_gain_db: f32,
    pub adaptive: AdaptiveDigital,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AdaptiveDigital {
    pub enabled: bool,
    pub headroom_db: f32,
    pub max_gain_db: f32,
    pub initial_gain_db: f32,
    pub max_gain_change_db_per_second: f32,
    pub max_output_noise_level_dbfs: f32,
}

impl Config {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        let adaptive = &self.adaptive;
        (0.0..50.0).contains(&self.fixed_gain_db)
            && adaptive.headroom_db >= 0.0
            && adaptive.max_gain_db > 0.0
            && adaptive.initial_gain_db >= 0.0
            && adaptive.max_gain_change_db_per_second > 0.0
            && adaptive.max_output_noise_level_dbfs <= 0.0
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            fixed_gain_db: 0.0,
            adaptive: AdaptiveDigital::default(),
        }
    }
}

impl Default for AdaptiveDigital {
    fn default() -> AdaptiveDigital {
        AdaptiveDigital {
            enabled: false,
            headroom_db: 5.0,
            max_gain_db: 50.0,
            initial_gain_db: 15.0,
            max_gain_change_db_per_second: 6.0,
            max_output_noise_level_dbfs: -50.0,
        }
    }
}
