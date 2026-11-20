use rikka_app::prelude::*;
use rikka_nix::Run;
use rikka_ui::search::Action;
use rikka_widgets::{column, row};

use crate::describe;
use crate::log::Log;
use crate::updates::{Link, Updates};

const SIDEBAR: f32 = 252.0;

const DIVIDER: f32 = 1.0;

const REASON_HEIGHT: f32 = 168.0;

const FIND_MARK: &str = "rikka-updates-find";

pub(crate) fn caption_bar(_: &Updates) -> Option<CaptionBar> {
    Some(
        caption()
            .leading(
                row! {
                    icon(MaterialSymbol::SYSTEM_UPDATE_ALT).extent(Length::px(16.0));
                    text(tr!("updates.title")).label().medium().no_wrap();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center),
            )
            .into(),
    )
}

pub(crate) fn view(updates: &Updates) -> Element {
    match updates.link() {
        Link::Connecting => container(spinner().extent(Length::px(20.0)))
            .center()
            .fill()
            .any(),
        Link::Failed(reason) => status_page(
            MaterialSymbol::CLOUD_OFF,
            tr!("updates.unavailable", reason = reason),
        )
        .action(
            button(text(tr!("updates.retry"))).on_press(|updates: &mut Updates| updates.start()),
        )
        .any(),
        Link::Ready if updates.runs().is_empty() => {
            status_page(MaterialSymbol::HISTORY, tr!("updates.empty")).any()
        }
        Link::Ready => row! {
            sidebar(updates);
            divider();
            pane(updates);
        }
        .fill()
        .any(),
    }
}

fn sidebar(updates: &Updates) -> impl View {
    let items: Vec<ListItem> = updates
        .runs()
        .iter()
        .map(|run| {
            let (symbol, tint) = describe::symbol(run);
            list_item(describe::title(run))
                .secondary(describe::when(run))
                .icon(symbol)
                .tint(tint)
                .busy(run.is_running())
        })
        .collect();
    container(
        list(items)
            .selected(updates.selected_index())
            .on_select(|updates: &mut Updates, index: usize| updates.select(index))
            .fill(),
    )
    .padding(Inset::all(Space::SNUG))
    .width(Length::px(SIDEBAR))
    .height(Length::FILL)
}

fn divider() -> impl View {
    container(spacer())
        .width(Length::px(DIVIDER))
        .height(Length::FILL)
        .background(palette().outline)
}

fn pane(updates: &Updates) -> Element {
    let Some(run) = updates.selected() else {
        return spacer().grow(1.0).any();
    };
    column! {
        header(run);
        updates.notice().map(|notice| notice_strip(notice.clone()));
        transcript(updates.log());
    }
    .gap(Space::NORMAL)
    .padding(Inset::all(Space::ROOMY))
    .grow(1.0)
    .min_width(0.0)
    .height(Length::FILL)
    .any()
}

fn header(run: &Run) -> impl View {
    let (symbol, tint) = describe::symbol(run);
    column! {
        row! {
            icon(symbol).extent(Length::px(24.0)).color(tint);
            column! {
                row! {
                    text(describe::title(run)).title().semibold().no_wrap();
                    text(describe::state(run)).caption().color(tint).no_wrap();
                }
                .gap(Space::SNUG)
                .align_items(Align::Center);
                text(describe::summary(run)).caption().muted();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            button(icon(MaterialSymbol::SEARCH).extent(Length::px(16.0)))
                .ghost()
                .small()
                .on_press(|log: &mut Log| log.open_find())
                .tooltip(tr!("updates.find"));
            button(text(tr!("updates.copy")))
                .ghost()
                .small()
                .on_press(|updates: &mut Updates| updates.copy_log());
            button(text(tr!("updates.save")))
                .small()
                .on_press(|updates: &mut Updates| updates.save_log());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        if run.is_running() {
            progress_block(run);
        }
        if let Some((reason, consequence)) = describe::failure(run) {
            failure_block(reason, consequence);
        }
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn progress_block(run: &Run) -> impl View {
    let bar = match run.status.fraction() {
        Some(fraction) => progress(fraction),
        None => progress_indeterminate(),
    };
    column! {
        row! {
            text(describe::phase(run)).label().no_wrap();
            spacer().grow(1.0);
            text(describe::work(run.status)).caption().muted().no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        bar.fill_width();
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn failure_block(reason: String, consequence: Label) -> impl View {
    container(
        column! {
            scrollable(
                text(reason)
                    .caption()
                    .family(Family::parse(font::JETBRAINS_MONO))
                    .color(palette().danger)
                    .fill_width(),
            )
            .max_height(REASON_HEIGHT)
            .fill_width();
            text(consequence).caption().muted();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .surface()
    .radius(Corner::Panel)
    .padding(Space::NORMAL)
    .fill_width()
}

fn notice_strip(notice: Label) -> impl View {
    container(
        row! {
            icon(MaterialSymbol::ERROR).extent(Length::px(16.0)).color(palette().danger);
            text(notice).caption().grow(1.0);
            button(text(tr!("updates.dismiss")))
                .ghost()
                .small()
                .on_press(|updates: &mut Updates| updates.dismiss_notice());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    )
    .surface()
    .radius(Corner::Panel)
    .padding(Inset::xy(12.0, 8.0))
    .fill_width()
}

fn transcript(log: &Log) -> impl View {
    let body = match (log.failure(), log.document().is_empty()) {
        (Some(reason), true) => status_page(
            MaterialSymbol::WARNING,
            tr!("updates.unreadable", reason = reason),
        )
        .any(),
        (None, true) if log.is_loading() => container(spinner().extent(Length::px(20.0)))
            .center()
            .fill()
            .any(),
        _ => column_of([
            find_mark(FIND_MARK),
            jump_to_end(
                text_editor(log.document())
                    .read_only(true)
                    .wrap(false)
                    .gutter(true)
                    .family(Family::parse(font::JETBRAINS_MONO))
                    .search(log.search())
                    .place(log.place())
                    .fill(),
                log.place(),
            )
            .fill()
            .any(),
            find_bar(log.search())
                .on_action(|log: &mut Log, action: Action| log.find(action))
                .anchored_to(FIND_MARK)
                .any(),
        ])
        .fill()
        .any(),
    };
    container(body)
        .surface()
        .radius(Corner::Panel)
        .clipped()
        .grow(1.0)
        .fill_width()
}
