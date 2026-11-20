use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::row;

use super::{Media, Notice, open_link};

pub(crate) fn notice_bar(notice: &Notice) -> Element {
    let folder = notice.folder.clone();
    container(
        row! {
            icon(MaterialSymbol::DOWNLOAD).extent(Length::px(16.0)).color(palette().accent);
            text(notice.caption.clone()).caption().grow(1.0).min_width(0.0);
            if let Some(folder) = folder {
                button(text(tr!("discord.conversation.show-folder")).caption())
                    .small()
                    .ghost()
                    .on_press(move || {
                        open_link(&folder.to_string_lossy());
                        skip_view();
                    });
            }
            button(icon(MaterialSymbol::CLOSE).extent(Length::px(14.0)))
                .icon_only()
                .ghost()
                .small()
                .on_press(|media: &mut Media| media.dismiss());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
    .background(palette().surface)
    .fill_width()
    .any()
}
