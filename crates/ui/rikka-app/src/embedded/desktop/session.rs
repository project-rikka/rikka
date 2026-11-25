use rikka_appearance::Appearance;

use crate::embedded::{Mail, Post};
use crate::wayland::settings::{self, SessionChange};

#[derive(Debug)]
pub(in crate::embedded) struct Following;

pub(in crate::embedded) fn follow(post: Post, session: Appearance) -> Following {
    settings::apply_session_locale();
    settings::follow(
        move |change| match change {
            SessionChange::Appearance { appearance, theme } => {
                post.send(Mail::Appearance { appearance, theme })
            }
            SessionChange::Language => post.send(Mail::Language),
        },
        Some(session),
        true,
    );
    Following
}
