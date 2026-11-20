use alacritty_terminal::event::Event;

#[derive(Clone, Debug)]
pub enum Report {
    Title(String),
    ResetTitle,
    Exited,
    Ask(Question),
}

#[derive(Clone, Debug)]
pub struct Question(pub(crate) Event);

impl Report {
    pub(crate) fn from_event(event: Event) -> Option<Self> {
        Some(match event {
            Event::Wakeup
            | Event::MouseCursorDirty
            | Event::CursorBlinkingChange
            | Event::Bell
            | Event::ClipboardStore(..)
            | Event::ClipboardLoad(..) => return None,
            Event::Title(title) => Self::Title(title),
            Event::ResetTitle => Self::ResetTitle,
            Event::Exit | Event::ChildExit(_) => Self::Exited,
            question @ (Event::PtyWrite(_)
            | Event::ColorRequest(..)
            | Event::TextAreaSizeRequest(_)) => Self::Ask(Question(question)),
        })
    }
}
