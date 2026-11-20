mod cell;
mod choices;
mod flag_or_off;
mod mail_settings;
mod requests;
mod setting;

pub(crate) use cell::SettingsCell;
pub use choices::{
    AutoReplyRepeat, ComposerMode, ImageProxy, MessageButtons, NextMessageOnMove, PgpScheme,
    PmSignature, ShowImages, ShowMoved, SpamAction, SwipeAction, ToolbarAction, ViewLayout,
    ViewMode,
};
pub use mail_settings::{AutoResponder, MailSettings, MobileSettings, Toolbar};
pub use setting::MailSetting;
