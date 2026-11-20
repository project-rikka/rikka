#[cfg(unix)]
mod clipboard;
#[cfg(unix)]
mod error;
mod watch;
#[cfg(windows)]
mod win32;
#[cfg(unix)]
mod worker;

#[cfg(unix)]
pub use clipboard::Clipboard;
pub use rikka_clipboard_core::{
    Error, Files, Offer, PNG, Result, Selection, Session, TEXT, URI_LIST, files_in, offers_files,
    text_mime, uri_list,
};
pub use watch::Change;
#[cfg(windows)]
pub use win32::Clipboard;
