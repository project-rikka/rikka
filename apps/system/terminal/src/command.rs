use rikka_widgets::Side;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Command {
    NewWindow,
    CloseWindow,
    Fullscreen,
    NewTab,
    DuplicateTab,
    CloseTab,
    NextTab,
    PreviousTab,
    Tab(usize),
    LastTab,
    Split(Side),
    SplitAuto,
    FocusPane(Side),
    ResizePane(Side),
    Find,
    Bigger,
    Smaller,
    ResetSize,
}
