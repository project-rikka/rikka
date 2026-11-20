use rikka_app::prelude::*;
use rikka_nix::{Document, Expr, Module};

use crate::pending::Pending;
use crate::settings::Settings;
use crate::users::component::Users;

const LIMIT: usize = 128;

const FIELD: f32 = 260.0;

const FORCE: &str = "lib.mkForce";

const LIB: &str = "lib";

pub(crate) fn view(users: &Users) -> impl View {
    let typed = users.name().text();
    let commit = Handler::new(|settings: &mut Settings| settings.rename_user());
    section(
        tr!("settings.users.name"),
        field(
            tr!("settings.users.full-name"),
            text_field(users.name())
                .max_len(LIMIT)
                .error(!is_valid(&typed))
                .on_change(|| {})
                .on_blur(commit.clone())
                .on_submit(commit)
                .width(FIELD),
        ),
    )
}

fn description(user: &str) -> [&str; 4] {
    ["users", "users", user, "description"]
}

pub(crate) fn staged(document: &Document, user: &str) -> Option<String> {
    match document.get::<Expr>(description(user)).ok().flatten()? {
        Expr::String(name) => Some(name),
        Expr::Apply { function, argument }
            if function
                .as_reference()
                .is_some_and(|reference| reference.to_string() == FORCE) =>
        {
            argument.as_str().map(str::to_owned)
        }
        _ => None,
    }
}

pub(crate) fn is_valid(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && name.chars().count() <= LIMIT
        && !name
            .chars()
            .any(|ch| matches!(ch, ':' | ',') || ch.is_control())
}

pub(crate) fn write(pending: &mut Pending, user: &str, name: &str, current: &str) {
    let name = name.trim();
    let edited = pending
        .edited(Module::System)
        .and_then(|document| staged(document, user));
    if !is_valid(name) || name == edited.as_deref().unwrap_or(current) {
        return;
    }
    let saved = pending
        .saved(Module::System)
        .and_then(|document| staged(document, user));
    if saved.is_none() && name == current {
        pending.change(Module::System, |document| {
            Ok(document.remove(description(user))?)
        });
        return;
    }
    pending.change(Module::System, |document| {
        document.add_formal(LIB)?;
        document.set(
            description(user),
            Expr::apply(Expr::reference(FORCE)?, name),
        )?;
        Ok(())
    });
}
