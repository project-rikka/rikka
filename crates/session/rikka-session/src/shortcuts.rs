use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use rikka_seam::{Event, Version};
use serde::de::{Error as _, Unexpected};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::profile::Profile;
use crate::protocol::Protocol;

pub const PROTOCOL: Protocol = Protocol::new("rikka.shortcuts", Version::new(1, 0));
pub const CAPABILITY: &str = PROTOCOL.capability();
pub const VERSION: Version = PROTOCOL.version();

pub const DOMAIN: &str = "rikka.input";

pub const ACTIVATE: &str = "activate";
pub const ACTIVATED: &str = "activated";

pub const WORKSPACES: u8 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    Session,
    Window,
    Columns,
    Workspace,
    Desktop,
}

impl Group {
    pub const ALL: [Group; 5] = [
        Group::Session,
        Group::Window,
        Group::Columns,
        Group::Workspace,
        Group::Desktop,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shortcut {
    Quit,
    StopDesktop,
    SpawnTerminal,
    CloseWindow,
    ToggleFullscreen,
    ToggleMaximize,
    ToggleFloating,
    SwitchWindows,
    SwitchWindowsBack,
    FocusColumnLeft,
    FocusColumnRight,
    FocusWindowUp,
    FocusWindowDown,
    MoveColumnLeft,
    MoveColumnRight,
    MoveWindowUp,
    MoveWindowDown,
    ConsumeOrExpelLeft,
    ConsumeOrExpelRight,
    SwitchColumnWidth,
    ShowWorkspace(u8),
    MoveToWorkspace(u8),
    ToggleOverview,
    EditDesk,
    ToggleLauncher,
    ToggleKeyboard,
    ClipboardHistory,
    EmojiPicker,
    ScreenshotScreen,
    ScreenshotRegion,
    RecordScreen,
    RecordRegion,
    SaveReplay,
    ChooseReplay,
    ToggleTranslate,
    TranslateRegion,
    ColorPicker,
    ScanCode,
    LockScreen,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShortcutInfo {
    pub name: Cow<'static, str>,
    pub chord: Cow<'static, str>,
    pub summary: Cow<'static, str>,
    pub group: Group,
}

const NAMED: [Shortcut; 37] = [
    Shortcut::Quit,
    Shortcut::StopDesktop,
    Shortcut::SpawnTerminal,
    Shortcut::CloseWindow,
    Shortcut::ToggleFullscreen,
    Shortcut::ToggleMaximize,
    Shortcut::ToggleFloating,
    Shortcut::SwitchWindows,
    Shortcut::SwitchWindowsBack,
    Shortcut::FocusColumnLeft,
    Shortcut::FocusColumnRight,
    Shortcut::FocusWindowUp,
    Shortcut::FocusWindowDown,
    Shortcut::MoveColumnLeft,
    Shortcut::MoveColumnRight,
    Shortcut::MoveWindowUp,
    Shortcut::MoveWindowDown,
    Shortcut::ConsumeOrExpelLeft,
    Shortcut::ConsumeOrExpelRight,
    Shortcut::SwitchColumnWidth,
    Shortcut::ToggleOverview,
    Shortcut::EditDesk,
    Shortcut::ToggleLauncher,
    Shortcut::ToggleKeyboard,
    Shortcut::ClipboardHistory,
    Shortcut::EmojiPicker,
    Shortcut::ScreenshotScreen,
    Shortcut::ScreenshotRegion,
    Shortcut::RecordScreen,
    Shortcut::RecordRegion,
    Shortcut::SaveReplay,
    Shortcut::ChooseReplay,
    Shortcut::ToggleTranslate,
    Shortcut::TranslateRegion,
    Shortcut::ColorPicker,
    Shortcut::ScanCode,
    Shortcut::LockScreen,
];

const WORKSPACE: &str = "workspace-";
const MOVE_TO_WORKSPACE: &str = "move-to-workspace-";

impl Shortcut {
    pub fn all() -> impl Iterator<Item = Shortcut> {
        NAMED.into_iter().chain(
            (1..=WORKSPACES)
                .flat_map(|number| [Self::ShowWorkspace(number), Self::MoveToWorkspace(number)]),
        )
    }

    #[must_use]
    pub fn group(self) -> Group {
        match self {
            Self::Quit | Self::StopDesktop | Self::SpawnTerminal => Group::Session,
            Self::CloseWindow
            | Self::ToggleFullscreen
            | Self::ToggleMaximize
            | Self::ToggleFloating
            | Self::SwitchWindows
            | Self::SwitchWindowsBack => Group::Window,
            Self::FocusColumnLeft
            | Self::FocusColumnRight
            | Self::FocusWindowUp
            | Self::FocusWindowDown
            | Self::MoveColumnLeft
            | Self::MoveColumnRight
            | Self::MoveWindowUp
            | Self::MoveWindowDown
            | Self::ConsumeOrExpelLeft
            | Self::ConsumeOrExpelRight
            | Self::SwitchColumnWidth => Group::Columns,
            Self::ShowWorkspace(_) | Self::MoveToWorkspace(_) => Group::Workspace,
            Self::ToggleOverview
            | Self::EditDesk
            | Self::ToggleLauncher
            | Self::ToggleKeyboard
            | Self::ClipboardHistory
            | Self::EmojiPicker
            | Self::SaveReplay
            | Self::ChooseReplay
            | Self::ScreenshotScreen
            | Self::ScreenshotRegion
            | Self::RecordScreen
            | Self::RecordRegion
            | Self::ToggleTranslate
            | Self::TranslateRegion
            | Self::ColorPicker
            | Self::ScanCode
            | Self::LockScreen => Group::Desktop,
        }
    }

    #[must_use]
    pub fn describe(self) -> ShortcutInfo {
        let (name, chord, summary) = match self {
            Self::Quit => ("quit", "Ctrl+Alt+Backspace", "Ends the session"),
            Self::StopDesktop => (
                "stop-desktop",
                "Ctrl+Alt+R",
                "Stops the desktop — the wallpaper, the taskbar and the launcher — and \
                 leaves every window where it is",
            ),
            Self::SpawnTerminal => ("spawn-terminal", "Super+Return", "Starts $TERMINAL"),
            Self::CloseWindow => (
                "close-window",
                "Super+Q",
                "Asks the focused window to close",
            ),
            Self::ToggleFullscreen => (
                "toggle-fullscreen",
                "Super+F",
                "Puts the focused window full screen, or takes it back",
            ),
            Self::ToggleMaximize => (
                "toggle-maximize",
                "Super+M",
                "Maximizes the focused window, or takes it back",
            ),
            Self::ToggleFloating => (
                "toggle-floating",
                "Super+Space",
                "Takes the focused window out of the tiling, or puts it back",
            ),
            Self::SwitchWindows => (
                "switch-windows",
                "Alt+Tab",
                "Goes back to the window used before; holding Alt, each Tab goes one window \
                 further back, and letting Alt go switches to the chosen one",
            ),
            Self::SwitchWindowsBack => (
                "switch-windows-back",
                "Alt+Shift+Tab",
                "Goes to the window used longest ago; holding Alt, each Tab goes one window \
                 more recent",
            ),
            Self::FocusColumnLeft => (
                "focus-column-left",
                "Super+Left",
                "Focuses the column to the left, scrolling it into view",
            ),
            Self::FocusColumnRight => (
                "focus-column-right",
                "Super+Right",
                "Focuses the column to the right, scrolling it into view",
            ),
            Self::FocusWindowUp => (
                "focus-window-up",
                "Super+Up",
                "Focuses the window above in the same column",
            ),
            Self::FocusWindowDown => (
                "focus-window-down",
                "Super+Down",
                "Focuses the window below in the same column",
            ),
            Self::MoveColumnLeft => (
                "move-column-left",
                "Super+Ctrl+Left",
                "Moves the focused column one place to the left",
            ),
            Self::MoveColumnRight => (
                "move-column-right",
                "Super+Ctrl+Right",
                "Moves the focused column one place to the right",
            ),
            Self::MoveWindowUp => (
                "move-window-up",
                "Super+Ctrl+Up",
                "Moves the focused window up within its column",
            ),
            Self::MoveWindowDown => (
                "move-window-down",
                "Super+Ctrl+Down",
                "Moves the focused window down within its column",
            ),
            Self::ConsumeOrExpelLeft => (
                "consume-or-expel-left",
                "Super+[",
                "Stacks a lone window into the column to the left, or takes a stacked one out \
                 into a column of its own on the left",
            ),
            Self::ConsumeOrExpelRight => (
                "consume-or-expel-right",
                "Super+]",
                "Stacks a lone window into the column to the right, or takes a stacked one out \
                 into a column of its own on the right",
            ),
            Self::SwitchColumnWidth => (
                "switch-column-width",
                "Super+Alt+R",
                "Widens the focused column to the next of a third, a half and two thirds of the \
                 display",
            ),
            Self::ToggleOverview => (
                "toggle-overview",
                "Super+Tab",
                "Shows every window and workspace at once, or takes it back",
            ),
            Self::EditDesk => (
                "edit-desk",
                "Super+W",
                "Shows the desktop's widgets and wallpapers to arrange, or takes them back",
            ),
            Self::ToggleLauncher => (
                "toggle-launcher",
                "Super+P",
                "Shows the desktop's launcher, or takes it back",
            ),
            Self::ToggleKeyboard => (
                "toggle-keyboard",
                "Super+K",
                "Shows the desktop's on-screen keyboard, or takes it back",
            ),
            Self::ClipboardHistory => (
                "clipboard-history",
                "Super+V",
                "Shows what has been on the clipboard, to put one of them back",
            ),
            Self::EmojiPicker => (
                "emoji-picker",
                "Super+.",
                "Shows the desktop's emoji picker, to put one where the typing is",
            ),
            Self::ScreenshotScreen => (
                "screenshot-screen",
                "Super+S",
                "Takes a picture of the whole screen",
            ),
            Self::ScreenshotRegion => (
                "screenshot-region",
                "Super+Shift+S",
                "Asks which rectangle or window to take a picture of",
            ),
            Self::RecordScreen => (
                "record-screen",
                "Super+R",
                "Starts recording the whole screen, or stops the recording that is running",
            ),
            Self::RecordRegion => (
                "record-region",
                "Super+Shift+R",
                "Asks which rectangle or window to record",
            ),
            Self::SaveReplay => (
                "save-replay",
                "Super+Alt+G",
                "Saves what the instant replay kept of what is in front: the focused window or \
                 the display it is on",
            ),
            Self::ChooseReplay => (
                "choose-replay",
                "Super+G",
                "Shows what the instant replay kept, to cut a part of it and save it",
            ),
            Self::ToggleTranslate => (
                "toggle-translate",
                "Super+T",
                "Shows the desktop's translation window, or takes it back",
            ),
            Self::TranslateRegion => (
                "translate-region",
                "Super+Shift+T",
                "Asks which rectangle or window to translate the text in",
            ),
            Self::ColorPicker => (
                "color-picker",
                "Super+Shift+C",
                "Picks a colour from the screen and copies it, or opens the colour editor",
            ),
            Self::ScanCode => (
                "scan-code",
                "Super+Shift+Q",
                "Freezes the screen and reads the QR codes and barcodes on it",
            ),
            Self::LockScreen => (
                "lock-screen",
                "Super+L",
                "Locks the screen until the password is typed again",
            ),
            Self::ShowWorkspace(number) => {
                return ShortcutInfo {
                    name: Cow::Owned(format!("{WORKSPACE}{number}")),
                    chord: Cow::Owned(format!("Super+{}", digit(number))),
                    summary: Cow::Owned(format!(
                        "Shows workspace {number} on the display the pointer is on"
                    )),
                    group: Group::Workspace,
                };
            }
            Self::MoveToWorkspace(number) => {
                return ShortcutInfo {
                    name: Cow::Owned(format!("{MOVE_TO_WORKSPACE}{number}")),
                    chord: Cow::Owned(format!("Super+Shift+{}", digit(number))),
                    summary: Cow::Owned(format!("Sends the focused window to workspace {number}")),
                    group: Group::Workspace,
                };
            }
        };
        ShortcutInfo {
            name: Cow::Borrowed(name),
            chord: Cow::Borrowed(chord),
            summary: Cow::Borrowed(summary),
            group: self.group(),
        }
    }

    #[must_use]
    pub fn is_published(self) -> bool {
        self.group() == Group::Desktop
    }

    #[must_use]
    pub fn is_available_in(self, profile: Profile) -> bool {
        profile.is_full()
            || !matches!(
                self.group(),
                Group::Columns | Group::Workspace | Group::Desktop
            ) && self != Self::ToggleFloating
    }
}

fn digit(number: u8) -> char {
    char::from_digit(u32::from(number % 10), 10).unwrap_or('0')
}

impl fmt::Display for Shortcut {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe().name)
    }
}

impl FromStr for Shortcut {
    type Err = UnknownShortcut;

    fn from_str(name: &str) -> Result<Self, UnknownShortcut> {
        let unknown = || UnknownShortcut(name.to_owned());
        if let Some(number) = name.strip_prefix(MOVE_TO_WORKSPACE) {
            return numbered(number)
                .map(Self::MoveToWorkspace)
                .ok_or_else(unknown);
        }
        if let Some(number) = name.strip_prefix(WORKSPACE) {
            return numbered(number)
                .map(Self::ShowWorkspace)
                .ok_or_else(unknown);
        }
        NAMED
            .into_iter()
            .find(|shortcut| shortcut.describe().name == name)
            .ok_or_else(unknown)
    }
}

fn numbered(number: &str) -> Option<u8> {
    number
        .parse()
        .ok()
        .filter(|number| (1..=WORKSPACES).contains(number))
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("no shortcut named `{0}`")]
pub struct UnknownShortcut(pub String);

impl Serialize for Shortcut {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.describe().name)
    }
}

impl<'de> Deserialize<'de> for Shortcut {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        name.parse().map_err(|_| {
            D::Error::invalid_value(Unexpected::Str(&name), &"the name of a session shortcut")
        })
    }
}

#[must_use]
pub fn activated(event: &Event) -> Option<Shortcut> {
    PROTOCOL.decode_event(event, ACTIVATED)
}
