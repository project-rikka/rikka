use std::path::Path;
use std::sync::Arc;

use ade_files::JobKind;
use rikka_app::prelude::*;
use rikka_file_operations::{Action, Error as Operation, Phase, Progress};
use rikka_format::RelativeTime;
use rikka_llm_accounts::Error as AccountError;

pub(crate) fn time_ago(at: u64) -> Label {
    match rikka_format::time_ago(at) {
        Some(relative) => span(relative),
        None => tr!("ade.ago.now"),
    }
}

fn span(relative: RelativeTime) -> Label {
    match relative {
        RelativeTime::Minutes(count) => tr!("ade.span.minutes", count = count as i64),
        RelativeTime::Hours(count) => tr!("ade.span.hours", count = count as i64),
        RelativeTime::Days(count) => tr!("ade.span.days", count = count as i64),
    }
}

pub(crate) fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub(crate) fn account_error(error: &AccountError) -> Arc<str> {
    match error {
        AccountError::Unconfigured => tr!("ade.chat.unconfigured").into(),
        AccountError::Keyless(account) => {
            tr!("ade.chat.keyless", provider = account.as_str()).into()
        }
        AccountError::SignedOut(account) => {
            tr!("ade.chat.signed-out", provider = account.as_str()).into()
        }
        AccountError::Modelless(account) => {
            tr!("ade.chat.modelless", provider = account.as_str()).into()
        }
        other => Arc::from(other.to_string()),
    }
}

pub(crate) fn file_error(error: &rikka_file_edit::Error) -> Arc<str> {
    match error {
        rikka_file_edit::Error::NotFound(_) => tr!("ade.file.gone").into(),
        rikka_file_edit::Error::NotText(_) => tr!("ade.file.not-text").into(),
        rikka_file_edit::Error::TooBig { .. } => tr!("ade.file.too-big").into(),
        rikka_file_edit::Error::ConcurrentModification(_) => tr!("ade.file.unwritable").into(),
        rikka_file_edit::Error::Io { reason, .. } => {
            tr!("ade.file.trouble", reason = reason).into()
        }
        other => tr!("ade.file.trouble", reason = &other.to_string()).into(),
    }
}

pub(crate) fn files_error(error: &ade_files::Error) -> Arc<str> {
    match error {
        ade_files::Error::Empty => tr!("ade.files.empty").into(),
        ade_files::Error::BadName(name)
        | ade_files::Error::Operation(Operation::BadName { name }) => {
            tr!("ade.files.bad-name", name = name).into()
        }
        ade_files::Error::Exists(path)
        | ade_files::Error::Operation(Operation::Exists { at: path }) => {
            tr!("ade.files.exists", name = &file_name(path)).into()
        }
        ade_files::Error::Outside(_) => tr!("ade.files.outside").into(),
        ade_files::Error::Operation(Operation::IntoItself { at }) => {
            tr!("ade.files.into-itself", name = &file_name(at)).into()
        }
        ade_files::Error::Operation(
            failure @ Operation::Unsupported {
                action: Action::Trash,
                ..
            },
        ) => tr!("ade.files.untrashed", reason = &failure.to_string()).into(),
        ade_files::Error::Operation(failure) => {
            tr!("ade.files.trouble", reason = &failure.to_string()).into()
        }
    }
}

pub(crate) fn files_failures(failures: &[ade_files::Error]) -> Option<Arc<str>> {
    let (first, rest) = failures.split_first()?;
    let first = files_error(first);
    Some(match rest.len() {
        0 => first,
        more => tr!("ade.files.and-more", first = &*first, count = more as i64).into(),
    })
}

pub(crate) fn job_status(kind: JobKind, progress: &Progress) -> String {
    let task = match kind {
        JobKind::Move => tr!("ade.files.job.move"),
        JobKind::Copy => tr!("ade.files.job.copy"),
        JobKind::Trash => tr!("ade.files.job.trash"),
        JobKind::Delete => tr!("ade.files.job.delete"),
    }
    .to_string();
    match progress.phase {
        Phase::Preparing => tr!("ade.files.job.preparing", task = &task),
        Phase::Working | Phase::Asking | Phase::Done => tr!(
            "ade.files.job.progress",
            task = &task,
            done = progress.items_done as i64,
            total = progress.items_total as i64,
        ),
    }
    .to_string()
}
