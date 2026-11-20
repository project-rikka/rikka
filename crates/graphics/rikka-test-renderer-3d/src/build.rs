use std::time::{Duration, Instant};

use rikka_renderer_3d::{MaterialTemplate, TemplateReadiness};

const OPTIMIZED_BUILD: Duration = Duration::from_secs(120);

pub fn built<T>(template: &MaterialTemplate, mut draw: impl FnMut() -> T) -> T {
    let start = Instant::now();
    loop {
        let readiness = template.readiness();
        let drawn = draw();
        match readiness {
            TemplateReadiness::Built => return drawn,
            TemplateReadiness::Failed(error) => panic!("the optimized build failed: {error}"),
            TemplateReadiness::Pending | TemplateReadiness::Drawable => {}
        }
        assert!(
            start.elapsed() < OPTIMIZED_BUILD,
            "{} never finished building",
            template.name()
        );
    }
}
