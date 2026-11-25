use std::sync::{Mutex, PoisonError};

use rikka_appearance::{Appearance, AppearanceWatch};

use crate::embedded::{Mail, Post};

#[derive(Debug)]
pub(in crate::embedded) struct Following {
    _watch: Option<AppearanceWatch>,
}

pub(in crate::embedded) fn follow(post: Post, session: Appearance) -> Following {
    let last = Mutex::new(session);
    let watch = AppearanceWatch::new(move || {
        let appearance = Appearance::current();
        {
            let mut last = last.lock().unwrap_or_else(PoisonError::into_inner);
            if *last == appearance {
                return;
            }
            last.clone_from(&appearance);
        }
        let theme = appearance.theme();
        post.send(Mail::Appearance {
            appearance: Box::new(appearance),
            theme: Some(Box::new(theme)),
        });
    });
    let watch = watch
        .inspect_err(|error| tracing::warn!("failed to watch the system appearance: {error}"))
        .ok();
    Following { _watch: watch }
}
