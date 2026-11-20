mod closure;
mod entry;
mod handles;
mod strings;

pub(crate) use closure::{AnyThread, Closure, ThisThread, required};
pub(crate) use entry::{enter, violation};
pub(crate) use handles::{
    free_owned, give_local, give_owned, give_shared, lend, local_ref, owned_mut, ref_local,
    ref_shared, scoped_mut, shared_ref, take_owned, unref_local, unref_shared,
};
pub(crate) use strings::{free_string, give_string, slice, text, view, views};

pub(crate) fn enumerated<E>(
    raw: u32,
    from_raw: fn(u32) -> Option<E>,
    type_name: &str,
    param: &str,
) -> E {
    from_raw(raw)
        .unwrap_or_else(|| violation(format!("`{param}` is not a valid {type_name}: {raw}")))
}
