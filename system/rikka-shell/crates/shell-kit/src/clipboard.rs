use std::sync::Arc;

use rikka_clipboard::Offer;

pub fn copy(text: &str) {
    rikka_ui::clipboard::set_text(text);
}

pub fn copy_picture(png: Arc<[u8]>) {
    if let Err(error) = rikka_ui::clipboard::publish(Offer::png(png)) {
        tracing::warn!("failed to copy picture to clipboard: {error}");
    }
}
