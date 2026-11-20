mod downloads;
mod error;
mod extensions;
mod notifications;
mod outbox;
mod preferences;
mod process;
mod runtime;
mod session;
mod sign_in;
mod storage;
mod surface;
mod threads;
mod uris;
mod wire;

pub use downloads::{Download, DownloadRequest, Downloads};
pub use error::{Error, Result};
pub use extensions::{
    ActionChange, ActionKind, ExtensionChange, ExtensionIcon, Extensions, InstallFailure,
    InstallRequest, Lifecycle, NewTabRequest, OptionalPermissionsRequest, OptionsPageRequest,
    PopupOpening, TabCloseRequest, TabUpdateRequest, UpdateRequest,
};
pub use notifications::Notification;
pub use preferences::{
    Autoplay, ColorScheme, CookiePolicy, DnsOverHttps, Endpoint, HttpsOnly, Locales, ManualProxy,
    PermissionDefault, Pref, Preferences, Proxy, SitePermission, Spellcheck, TrackingProtection,
};
pub use process::profile::Profile;
pub use process::supervise::{Exit, wait_for_closed_runtimes};
pub use runtime::{OpenWindowRequest, Runtime, RuntimeBuilder, RuntimeEvent};
pub use session::{
    Alert, AllowPopup, AllowRedirect, Button, ButtonLabel, Check, Choose, CloseReason, Confirm,
    ConfirmFolderUpload, ConfirmLeave, ConfirmRepost, ContextMenu, Dropdown, Event,
    FailedCertificate, Find, Hang, HistoryEntry, HostVisitedQuery, Load, LoadError, LoadRequest,
    MediaCapture, MediaCommand, MediaRequest, MediaState, MenuTarget, MessageHandler,
    NewSessionRequest, OpenLink, PageState, PermissionRequest, PickColor, PickDateTime, PickFiles,
    Playback, Popup, Prompt, SaveLogin, SavedState, ScriptMessage, ScriptReply, Security,
    SelectAutofill, Session, SessionSettings, Share, SignIn, Snapshot, Spelling, TextInput, Trust,
    UserContent, UserScript, UserStyle, Visit, VisitedQuery, World,
};
pub use sign_in::SignInPortal;
pub use storage::{Partition, Storage};
pub use surface::{NavigationButton, WebSurface, WebSurfaceState, web_view};

pub use rikka_gecko_protocol::geckoview::autocomplete::{
    Address, CreditCard, LoginEntry, LoginSaveHint, SelectHint, SelectOption,
};
pub use rikka_gecko_protocol::geckoview::content_blocking::ContentBlockingEvent;
pub use rikka_gecko_protocol::geckoview::find::{DisplayMatches, FindFlags, FindResult};
pub use rikka_gecko_protocol::geckoview::geometry::Rect as ClientRect;
pub use rikka_gecko_protocol::geckoview::history::VisitFlags;
pub use rikka_gecko_protocol::geckoview::media::{MediaImage, MediaMetadata, PositionState};
pub use rikka_gecko_protocol::geckoview::navigation::{
    ErrorCategory, ErrorClass, ErrorCode, LoadFlags, OpenFlags, OpenWhere, WebRequestError,
};
pub use rikka_gecko_protocol::geckoview::permission::{
    ContentPermission, MediaSource, PermissionKind, PermissionValue,
};
pub use rikka_gecko_protocol::geckoview::progress::{IdentityMode, MixedMode};
pub use rikka_gecko_protocol::geckoview::prompt::{
    AuthFlags, ButtonType, CaptureType, Choice, ChoiceMode, DateTimeMode, FileMode, PromptId,
    ShareResult,
};
pub use rikka_gecko_protocol::geckoview::session::Module;
pub use rikka_gecko_protocol::geckoview::storage::ClearFlags;
pub use rikka_gecko_protocol::geckoview::webextension::action::Action as ExtensionAction;
pub use rikka_gecko_protocol::geckoview::webextension::extension::{
    DisabledFlag, Extension, ExtensionFlags, MetaData as ExtensionMetaData, PermissionSet,
};
pub use rikka_gecko_protocol::geckoview::webextension::tab::{CreateTabDetails, UpdateTabDetails};
pub use rikka_gecko_protocol::rikka::capture::CaptureState;
pub use rikka_gecko_protocol::rikka::downloads::{DownloadId, DownloadReason, DownloadState};
pub use rikka_gecko_protocol::rikka::dropdown::{
    ChosenWith, DropdownGroup, DropdownId, DropdownItem, DropdownOption, UpdateDropdown,
};
pub use rikka_gecko_protocol::rikka::editing::EditCommand;
pub use rikka_gecko_protocol::rikka::notifications::NotificationAction;
pub use rikka_gecko_protocol::rikka::storage::{Cookie, SameSite, SiteData};
pub use rikka_gecko_protocol::rikka::user_content::{Frames, InjectAt, StyleLevel};
pub use rikka_gecko_protocol::rikka::view::{Icon, SnapshotRegion, Tooltip};
