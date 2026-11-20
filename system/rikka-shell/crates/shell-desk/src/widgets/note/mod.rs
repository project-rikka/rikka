mod drafts;

use desk_layout::{Note, Size, WidgetId};
pub(crate) use drafts::Drafts;
use rikka_ui::{Element, Family, Inset, Label, Styled, TextStyled, View, text, tr};
use rikka_widgets::{container, text_editor};

use crate::widgets::Mode;
use crate::widgets::card::INSET;
use crate::widgets::live::Live;
use crate::widgets::sources::Sources;

const EDITOR_TOP: f32 = 8.0;
const EDITOR_SIDE: f32 = 12.0;
const LEADING: f32 = 1.45;
const HANDWRITING: &str = "sans-serif";

#[must_use]
pub(crate) fn view(
    id: WidgetId,
    note: &Note,
    size: Size,
    sources: &Sources,
    mode: Mode,
) -> Element {
    let font = match size {
        Size::Small => 13.0,
        Size::Medium => 14.0,
        Size::Large => 15.0,
        Size::ExtraLarge => 17.0,
    };
    let line = (font * LEADING).round();
    let placeholder = tr!("shell.desk.widget.note.placeholder");
    match mode {
        Mode::Live => {
            let document = sources.drafts.document(id, &note.text);
            container(
                text_editor(&document)
                    .gutter(false)
                    .wrap(true)
                    .family(Family::parse(HANDWRITING))
                    .font_size(font)
                    .line_height(line)
                    .placeholder(placeholder)
                    .on_change(move |live: &mut Live| live.typed(id))
                    .on_blur(move |live: &mut Live| live.keep(id))
                    .fill(),
            )
            .padding(Inset::exact(
                INSET.px() - EDITOR_TOP,
                INSET.px() - EDITOR_SIDE,
                INSET.px() - EDITOR_TOP,
                INSET.px() - EDITOR_SIDE,
            ))
            .fill()
            .build()
        }
        Mode::Shown => {
            let note_text = match note.text.is_empty() {
                true => text(placeholder).faint(),
                false => text(Label::from(note.text.as_str())),
            };
            container(
                note_text
                    .family(Family::parse(HANDWRITING))
                    .text_size(font)
                    .line_height(line)
                    .fill_width(),
            )
            .padding(INSET)
            .clip_content()
            .fill()
            .build()
        }
    }
}
