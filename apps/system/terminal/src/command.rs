use rikka_widgets::Side;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    NewWindow,
    CloseWindow,
    Fullscreen,
    NewTab,
    CloseTab,
    NextTab,
    PreviousTab,
    Tab(usize),
    Split(Side),
    FocusPane(Side),
    Find,
    Bigger,
    Smaller,
    ResetSize,
}
