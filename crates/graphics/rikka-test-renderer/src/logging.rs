use std::sync::Once;

use tracing_subscriber::filter::LevelFilter;

pub fn init_tracing() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(LevelFilter::WARN)
            .with_test_writer()
            .init();
    });
}
