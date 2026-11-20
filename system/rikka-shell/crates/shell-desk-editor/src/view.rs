use std::sync::Arc;

use rikka_ui::{Length, Styled, View, component};
use rikka_widgets::spacer;

use crate::editor_canvas::{Sheet, editor_canvas};
use crate::overlay::Overlay;
use crate::stage::Editor;
use crate::{card, panel};

pub(crate) fn view(editor: &Editor) -> impl View {
    let output = editor.output();
    let area = editor.area();
    let layout = editor.layout();
    let selected = editor.selected().map(|widget| widget.id);
    let taskbar = editor.taskbar();
    let (thumbs, current) = editor.thumbs();
    let overlay = Overlay {
        session: editor.session().clone(),
        area,
        layout: layout.clone(),
        selected,
        taskbar,
    }
    .build()
    .width(Length::px(output.width))
    .height(Length::px(output.height))
    .position(0.0, 0.0);
    let panel = match editor.has_arrived() {
        true => panel::view(editor),
        false => spacer().position(0.0, 0.0).build(),
    };
    let sheet = Sheet {
        area,
        layout,
        editable: editor.editable(),
        selected,
        thumbs,
        current,
        pictures: editor.pictures(),
        taskbar,
        panel: editor.panel(),
        session: editor.session().clone(),
        editing: editor.editing().clone(),
        behind: editor.behind(),
        frost: editor.frost().clone(),
        output: Arc::from(editor.output_name()),
    };
    editor_canvas(
        sheet,
        vec![
            component(|editor: &Editor| editor, card::view),
            overlay,
            panel,
        ],
    )
}
