use browser_store::Decision;
use rikka_web::{Partition, PermissionValue, Runtime};

use browser_model::{Ask, Grants, Requester, Scope};

pub(crate) fn share(
    runtime: &Runtime,
    requester: &Requester,
    ask: Ask,
    decision: Option<Decision>,
) {
    let Some(name) = ask.engine_name(requester) else {
        return;
    };
    let value = match decision {
        Some(Decision::Allow) => PermissionValue::Allow,
        Some(Decision::Deny) => PermissionValue::Deny,
        None => PermissionValue::Unknown,
    };
    let partition = Partition {
        context: None,
        private: runtime.is_ephemeral(),
    };
    runtime
        .storage()
        .set_permission_for(requester.engine_uri(), name, value, &partition);
}

pub(crate) fn share_all(runtime: &Runtime, grants: &Grants, scope: Scope) {
    for (requester, ask, decision) in grants.decided_in(scope) {
        share(runtime, &requester, ask, Some(decision));
    }
}
