use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::menus::Subject;
use crate::shell::Shell;
use crate::sidebar::face::Face;
use crate::sidebar::look::{GAP, PITCH, ROW};
use crate::sidebar::row::{Line, RowTarget, row};
use crate::widgets::{DropLayout, DropTarget, drop_list, grip};
use browser_model::{Row, TabId};

pub(crate) fn view(shell: &Shell) -> Element {
    let model = shell.model();
    let Some(layout) = model.window(shell.id()) else {
        return Element::empty();
    };
    let tabs = layout.today_here().to_vec();
    let header = row! {
        text(tr!("browser.sidebar.today")).caption().muted().semibold().grow(1.0);
        (!tabs.is_empty()).then(|| {
            button(text(tr!("browser.sidebar.clear")).caption())
                .ghost()
                .small()
                .focusless()
                .on_press(|shell: &mut Shell| shell.clear_today())
        });
    }
    .align_items(Align::Center)
    .padding(Inset::edges(
        Space::SNUG,
        Space::SNUG,
        Space::NONE,
        Space::SNUG,
    ))
    .height(ROW)
    .fill_width();
    let entries = crate::split::Entries::collect(&model, shell.id()).shown;
    let rows = entries.iter().map(|entry| match *entry {
        crate::split::Entry::Tab(tab) => {
            animated_position(component_with(|shell: &Shell| shell, tab, line)).keyed(tab)
        }
        crate::split::Entry::Group(group) => animated_position(component_with(
            |shell: &Shell| shell,
            group,
            crate::split::sidebar_line,
        ))
        .keyed(("split", group)),
    });
    column! {
        new_tab();
        header;
        drop_list(
            DropLayout::Rows {
                pitch: PITCH,
                folders: vec![false; entries.len()],
            },
            entries.len(),
            column_of(rows).gap(GAP).fill_width(),
        )
        .on_drop(|shell: &mut Shell, row: Row, target: DropTarget| {
            shell.dropped_in_today(row, target.index);
        })
        .min_height(ROW)
        .fill_width();
    }
    .fill_width()
    .any()
}

fn line(shell: &Shell, tab: &TabId) -> Element {
    let Some(face) = Face::for_open_tab(&shell.model(), shell.id(), *tab) else {
        return Element::empty();
    };
    row(Line {
        face,
        depth: 0,
        opens: RowTarget::Tab(*tab),
        subject: Subject::Tab(*tab),
        row: Row::Tab(*tab),
        tears_off: true,
    })
}

fn new_tab() -> Element {
    grip(
        row! {
            icon(MaterialSymbol::ADD).extent(Length::px(16.0)).color(palette().text_muted);
            text(tr!("browser.sidebar.new-tab")).label().muted().no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::xy(Space::SNUG, Space::NONE))
        .fill(),
    )
    .on_press(|shell: &mut Shell| shell.new_tab())
    .height(ROW)
    .fill_width()
    .any()
}
