pub const OVERVIEW: &str = "overview";

pub const DESK: &str = "desk";

pub const SCRIM: &str = "scrim";

pub const INPUT_PANEL: &str = "input-panel";

#[must_use]
pub fn hides_windows(namespace: &str) -> bool {
    matches!(namespace, OVERVIEW | DESK)
}

#[must_use]
pub fn is_scrim(namespace: &str) -> bool {
    namespace == SCRIM
}

#[must_use]
pub fn is_input_panel(namespace: &str) -> bool {
    namespace == INPUT_PANEL
}
