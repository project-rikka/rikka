use rikka_app::prelude::*;
use rikka_nix::Generation;
use rikka_widgets::column;

use crate::job::Job;
use crate::manager::Manager;
use crate::pane;
use crate::work::Work;

pub(crate) fn view(generations: &[Generation], work: &Work) -> impl View {
    scrollable(
        container(
            column! {
                for generation in generations {
                    entry(generation, work);
                }
            }
            .gap(Space::SNUG)
            .fill_width(),
        )
        .padding(Inset::xy(14.0, 12.0))
        .fill_width(),
    )
    .fill()
}

fn entry(generation: &Generation, work: &Work) -> impl View {
    let number = generation.number;
    pane::card(
        row! {
            icon(match generation.current {
                true => MaterialSymbol::CHECK_CIRCLE,
                false => MaterialSymbol::HISTORY,
            })
            .extent(Length::px(18.0))
            .color(match generation.current {
                true => palette().success,
                false => palette().text_muted,
            });
            text(format!("#{number}")).label().semibold().no_wrap();
            text(rikka_format::format_timestamp(generation.build_time))
                .caption()
                .muted()
                .no_wrap();
            if generation.version.is_empty() {
                spacer().grow(1.0);
            } else {
                text(generation.version.clone())
                    .caption()
                    .muted()
                    .no_wrap()
                    .grow(1.0);
            }
            if generation.current {
                pane::badge(
                    tr!("packages.generation.current").to_string(),
                    palette().success,
                );
            } else {
                match work.running(Job::Rollback(number)) {
                    Some(stage) => button(pane::working(stage)).small().enabled(false),
                    None => button(text(tr!("packages.action.roll-back")))
                        .small()
                        .enabled(!work.is_busy())
                        .on_press(move |manager: &mut Manager| manager.roll_back(number)),
                };
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
}
