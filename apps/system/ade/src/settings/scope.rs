use ade_scope::Scope;
use rikka_app::prelude::*;

pub(crate) fn whose(scope: Scope) -> Label {
    match scope {
        Scope::Project => tr!("ade.settings.scope.here"),
        Scope::User => tr!("ade.settings.scope.mine"),
    }
}
