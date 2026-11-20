mod ade;
mod hint;
mod labels;
mod notice;
mod project;
mod screen;
mod settings;
mod setup;
mod welcome;

use rikka_app::prelude::*;

use crate::ade::Ade;

rikka_app::mimalloc!();

fn main() -> rikka_app::Result<()> {
    App::new("rikka-ade")
        .i18n(
            rikka_i18n::embedded!()
                .embedded(rikka_chat::CATALOGS)
                .embedded(rikka_llm_accounts::CATALOGS)
                .embedded(rikka_llm_settings::CATALOGS)
                .embedded(rikka_usage_bar::CATALOGS),
        )
        .windows(|_: LaunchRequest| window())
        .run()
}

fn window() -> Window<Ade> {
    Window::new(Ade::new, ade::view)
        .title(tr!("ade.title"))
        .size(1180.0, 780.0)
        .min_size(880.0, 560.0)
        .on_start(Ade::start)
        .caption(ade::bar)
}
