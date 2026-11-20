use nixpkgs_search::Stage;
use rikka_app::prelude::*;
use rikka_widgets::column;

use super::Manager;
use super::index::Index;
use crate::job::Job;
use crate::list::Packages;
use crate::pane::{self, DETAIL};
use crate::section::Section;
use crate::{apps_page, detail, generations};

const SIDEBAR: f32 = 196.0;
const SEARCH: f32 = 340.0;
const PROGRESS: f32 = 240.0;

pub(crate) fn view(manager: &Manager) -> impl View {
    column! {
        row! {
            sidebar(manager);
            page_stack(manager.section.at(), middle(manager))
                .grow(1.0)
                .fill_height();
            detail(manager);
        }
        .grow(1.0);
        panels(manager);
    }
    .fill()
}

pub(crate) fn caption_bar(manager: &Manager) -> Option<CaptionBar> {
    let mut bar = caption()
        .leading(
            row! {
                icon(MaterialSymbol::PACKAGE_2).extent(Length::px(16.0));
                text(tr!("packages.title")).label().medium().no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .trailing(
            row! {
                match manager.work.running(Job::Update) {
                    Some(stage) => button(pane::working(stage)).ghost().small().enabled(false),
                    None => button(
                        row! {
                            icon(MaterialSymbol::SYSTEM_UPDATE_ALT).extent(Length::px(15.0));
                            text(tr!("packages.action.update")).caption().no_wrap();
                        }
                        .gap(Space::TIGHT)
                        .align_items(Align::Center),
                    )
                    .ghost()
                    .small()
                    .enabled(!manager.work.is_busy())
                    .on_press(|manager: &mut Manager| manager.update_system()),
                };
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        );
    if manager.section == Section::Search {
        bar = bar.center(
            text_field(manager.search.query())
                .placeholder(tr!("packages.search"))
                .claims_focus(manager.search_focused)
                .on_blur(Manager::search_blurred)
                .control_size(ControlSize::Small)
                .on_change(|manager: &mut Manager| manager.query_changed())
                .width(SEARCH),
        );
    }
    Some(bar.into())
}

fn sidebar(manager: &Manager) -> impl View {
    row! {
        container(
            nav_list(manager.section.at())
                .items(Section::ALL.map(Section::nav_item))
                .on_select(|manager: &mut Manager, index: usize| {
                    manager.open_section(Section::ALL.get(index).copied().unwrap_or_default());
                })
                .fill_width(),
        )
        .padding(Inset::xy(8.0, 12.0))
        .width(SIDEBAR)
        .fill_height();
        rule_upright();
    }
}

fn middle(manager: &Manager) -> Element {
    if manager.section == Section::Apps {
        return apps_page::view(manager.plan.apps(), manager.work.is_busy());
    }
    if manager.section == Section::Generations {
        if manager.generations.is_empty() {
            return container(notice(manager)).center().fill().build();
        }
        return generations::view(&manager.generations, &manager.work).build();
    }

    column! {
        bar(manager);
        if manager.listing_view.listing().is_empty() {
            container(notice(manager)).center().fill();
        } else {
            Packages::new(manager.listing_view.listing().clone(), manager.listing_view.selected())
                .view()
                .fill()
                .build()
                .keyed(manager.listing_view.revision());
        }
    }
    .fill()
    .build()
}

fn bar(manager: &Manager) -> impl View {
    let count = manager.listing_view.listing().len().to_string();
    let found = text(tr!("packages.found", count = &count))
        .caption()
        .muted()
        .no_wrap();
    let mut bar = toolbar()
        .padding(Inset::xy(14.0, 8.0))
        .group(tool_group([tool_item(found)]).priority(ToolPriority::Pinned));
    if let Some(channel) = manager.backend.channel() {
        let badge = pane::badge(
            tr!("packages.channel", name = channel.name()).to_string(),
            palette().text_muted,
        );
        bar = bar.group(tool_group([tool_item(badge)]).priority(ToolPriority::Low));
    }
    bar = bar.spacer();
    if let Some(catalog) = manager.index.catalog() {
        let release = text(catalog.release().to_owned())
            .caption()
            .muted()
            .no_wrap();
        bar = bar.group(tool_group([tool_item(release)]).priority(ToolPriority::Low));
    }
    bar
}

fn notice(manager: &Manager) -> StatusPage {
    if manager.section == Section::Search
        && let Some(indexing) = indexing(&manager.index)
    {
        return indexing;
    }
    if let Some(error) = &manager.error {
        return status_page(MaterialSymbol::ERROR, error.to_string()).tint(palette().danger);
    }
    let typed = match manager.section {
        Section::Search => manager.search.query_text(),
        _ => None,
    };
    match typed {
        Some(query) => status_page(
            MaterialSymbol::SEARCH_OFF,
            tr!("packages.empty.search", query = &query),
        ),
        None => status_page(manager.section.symbol(), manager.section.empty()),
    }
}

fn indexing(index: &Index) -> Option<StatusPage> {
    let stage = match index {
        Index::Ready(_) => return None,
        Index::Failed(error) => {
            return Some(
                status_page(MaterialSymbol::ERROR, error.to_string())
                    .tint(palette().danger)
                    .action(
                        button(text(tr!("packages.action.retry")))
                            .small()
                            .on_press(|manager: &mut Manager| manager.retry()),
                    ),
            );
        }
        Index::Building(stage) => *stage,
    };
    let (detail, done) = match stage {
        Stage::Locating => (tr!("packages.index.locating"), None),
        Stage::Downloading {
            received,
            total: Some(total),
        } if total > 0 => (
            tr!(
                "packages.index.downloading",
                received = &rikka_format::format_bytes(received),
                total = &rikka_format::format_bytes(total)
            ),
            Some(received as f32 / total as f32),
        ),
        Stage::Downloading { received, .. } => (
            tr!(
                "packages.index.receiving",
                received = &rikka_format::format_bytes(received)
            ),
            None,
        ),
        Stage::Indexing => (tr!("packages.index.indexing"), None),
    };
    let bar = match done {
        Some(done) => progress(done),
        None => progress_indeterminate(),
    };
    Some(status_page(MaterialSymbol::DOWNLOAD, detail).action(bar.width(PROGRESS)))
}

fn detail(manager: &Manager) -> Option<Element> {
    let (at, line, package) = manager.listing_view.selected_row()?;
    Some(
        row! {
            rule_upright();
            container(
                page_stack(at, detail::view(&line, package, manager.work.is_busy()))
                    .named(line.attribute)
                    .fill_width()
                    .fill_height(),
            )
            .width(DETAIL)
            .fill_height();
        }
        .fill_height()
        .build(),
    )
}

fn unreachable(manager: &Manager) -> Option<Element> {
    let error = manager.backend.unavailable()?;
    Some(
        pane::strip(
            MaterialSymbol::ERROR,
            palette().warning,
            error.to_string(),
            button(text(tr!("packages.action.retry")))
                .small()
                .on_press(|manager: &mut Manager| manager.retry()),
        )
        .build(),
    )
}

fn refused(manager: &Manager) -> Option<Element> {
    let reason = manager.work.refusal()?.to_string();
    Some(
        pane::strip(
            MaterialSymbol::ERROR,
            palette().danger,
            tr!("packages.work.not-started", reason = &reason),
            button(text(tr!("packages.action.close")))
                .ghost()
                .small()
                .on_press(|manager: &mut Manager| manager.dismiss_refusal()),
        )
        .build(),
    )
}

fn panels(manager: &Manager) -> Option<Element> {
    let panels: Vec<Element> = [
        unreachable(manager),
        refused(manager),
        manager.plan.panel(&manager.work),
    ]
    .into_iter()
    .flatten()
    .collect();
    if panels.is_empty() {
        return None;
    }
    Some(
        container(column_of(panels).gap(Space::SNUG).fill_width())
            .padding(Inset::xy(14.0, 12.0))
            .fill_width()
            .build(),
    )
}
