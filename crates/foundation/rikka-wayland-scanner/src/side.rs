#[derive(Debug, Clone, Copy)]
pub(crate) struct Side {
    pub(crate) incoming: &'static str,
    pub(crate) typed: &'static str,
    pub(crate) untyped: &'static str,
    pub(crate) from_untyped: &'static str,
    pub(crate) as_untyped: &'static str,
    pub(crate) objects: &'static str,
    pub(crate) binds: bool,
    pub(crate) owned: bool,
}

pub(crate) const SERVER: Side = Side {
    incoming: "Request",
    typed: "wire::Resource",
    untyped: "wire::Object",
    from_untyped: "from_object",
    as_untyped: "object",
    objects: "wire::Objects<'_>",
    binds: true,
    owned: false,
};

pub(crate) const CLIENT: Side = Side {
    incoming: "Event",
    typed: "wire::client::Proxy",
    untyped: "wire::client::Handle",
    from_untyped: "from_handle",
    as_untyped: "handle",
    objects: "wire::client::Resolved<'_>",
    binds: false,
    owned: true,
};
