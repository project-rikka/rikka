mod inhibitor;
mod shortcuts;

pub(crate) use inhibitor::ShortcutInhibitor;
pub(crate) use shortcuts::Shortcuts;

pub(crate) fn flush(connection: &rikka_wayland_client::Connection) {
    if let Err(error) = connection.flush() {
        tracing::debug!("failed to flush keyboard inhibitor: {error}");
    }
}
