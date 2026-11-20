use rikka_app::prelude::*;

pub(crate) fn info_row(label: Label, value: String) -> Option<Field> {
    if value.is_empty() {
        return None;
    }
    Some(field(label, text(value).label().align(TextAlign::End)))
}
