use desk_layout::Size;
use rikka_ui::{Label, tr};

#[must_use]
pub(crate) fn short(size: Size) -> Label {
    match size {
        Size::Small => tr!("shell.desk.editor.size.small"),
        Size::Medium => tr!("shell.desk.editor.size.medium"),
        Size::Large => tr!("shell.desk.editor.size.large"),
        Size::ExtraLarge => tr!("shell.desk.editor.size.extra-large"),
    }
}

#[must_use]
pub(crate) fn named(size: Size) -> Label {
    match size {
        Size::Small => tr!("shell.desk.editor.size-name.small"),
        Size::Medium => tr!("shell.desk.editor.size-name.medium"),
        Size::Large => tr!("shell.desk.editor.size-name.large"),
        Size::ExtraLarge => tr!("shell.desk.editor.size-name.extra-large"),
    }
}
