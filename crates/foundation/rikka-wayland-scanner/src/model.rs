#[derive(Debug)]
pub(crate) struct Protocol {
    pub(crate) interfaces: Vec<Interface>,
}

#[derive(Debug)]
pub(crate) struct Interface {
    pub(crate) name: String,
    pub(crate) version: u32,
    pub(crate) frozen: bool,
    pub(crate) requests: Vec<Message>,
    pub(crate) events: Vec<Message>,
    pub(crate) enums: Vec<Enum>,
}

#[derive(Debug)]
pub(crate) struct Message {
    pub(crate) name: String,
    pub(crate) since: u32,
    pub(crate) destructor: bool,
    pub(crate) args: Vec<Arg>,
}

#[derive(Debug)]
pub(crate) struct Arg {
    pub(crate) name: String,
    pub(crate) kind: ArgKind,
    pub(crate) interface: Option<String>,
    pub(crate) nullable: bool,
    pub(crate) enumeration: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArgKind {
    Int,
    Uint,
    Fixed,
    String,
    Object,
    NewId,
    Array,
    Fd,
}

#[derive(Debug)]
pub(crate) struct Enum {
    pub(crate) name: String,
    pub(crate) bitfield: bool,
    pub(crate) entries: Vec<Entry>,
}

#[derive(Debug)]
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) value: u32,
    pub(crate) summary: Option<String>,
}
