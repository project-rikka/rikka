use rikka_proton_api::wire_enum;
use serde::{Deserialize, Serialize};

wire_enum! {
    #[non_exhaustive]
    pub enum ComposerMode {
        Popup = 0,
        Maximized = 1,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum MessageButtons {
        ReadFirst = 0,
        UnreadFirst = 1,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum ShowImages {
        None = 0,
        Remote = 1,
        Embedded = 2,
        Both = 3,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum ShowMoved {
        None = 0,
        Drafts = 1,
        Sent = 2,
        Both = 3,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum ViewMode {
        Conversations = 0,
        Messages = 1,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum ViewLayout {
        Column = 0,
        Row = 1,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum SwipeAction {
        None = -1,
        Trash = 0,
        Spam = 1,
        Star = 2,
        Archive = 3,
        MarkRead = 4,
        LabelAs = 5,
        MoveTo = 6,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum PmSignature {
        Disabled = 0,
        Enabled = 1,
        Locked = 2,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum PgpScheme {
        Inline = 8,
        Mime = 16,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum NextMessageOnMove {
        Disabled = 0,
        Default = 1,
        Enabled = 2,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum SpamAction {
        JustSpam = 0,
        SpamAndUnsubscribe = 1,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum ImageProxy {
        None = 0,
        Incorporator = 1,
        Proxy = 2,
        All = 3,
    }
}

wire_enum! {
    #[non_exhaustive]
    pub enum AutoReplyRepeat {
        Fixed = 0,
        Daily = 1,
        Weekly = 2,
        Monthly = 3,
        Permanent = 4,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
#[non_exhaustive]
pub enum ToolbarAction {
    Reply,
    Forward,
    ToggleRead,
    ToggleStar,
    Label,
    Move,
    Trash,
    Archive,
    Spam,
    ToggleLight,
    Print,
    ReportPhishing,
    Other(String),
}

impl ToolbarAction {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            ToolbarAction::Reply => "reply",
            ToolbarAction::Forward => "forward",
            ToolbarAction::ToggleRead => "toggle_read",
            ToolbarAction::ToggleStar => "toggle_star",
            ToolbarAction::Label => "label",
            ToolbarAction::Move => "move",
            ToolbarAction::Trash => "trash",
            ToolbarAction::Archive => "archive",
            ToolbarAction::Spam => "spam",
            ToolbarAction::ToggleLight => "toggle_light",
            ToolbarAction::Print => "print",
            ToolbarAction::ReportPhishing => "report_phishing",
            ToolbarAction::Other(action) => action,
        }
    }
}

impl From<String> for ToolbarAction {
    fn from(action: String) -> ToolbarAction {
        match action.as_str() {
            "reply" => ToolbarAction::Reply,
            "forward" => ToolbarAction::Forward,
            "toggle_read" => ToolbarAction::ToggleRead,
            "toggle_star" => ToolbarAction::ToggleStar,
            "label" => ToolbarAction::Label,
            "move" => ToolbarAction::Move,
            "trash" => ToolbarAction::Trash,
            "archive" => ToolbarAction::Archive,
            "spam" => ToolbarAction::Spam,
            "toggle_light" => ToolbarAction::ToggleLight,
            "print" => ToolbarAction::Print,
            "report_phishing" => ToolbarAction::ReportPhishing,
            _ => ToolbarAction::Other(action),
        }
    }
}

impl From<ToolbarAction> for String {
    fn from(action: ToolbarAction) -> String {
        action.as_str().to_owned()
    }
}
