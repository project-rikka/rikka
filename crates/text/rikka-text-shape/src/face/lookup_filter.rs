use crate::face::digest::Digest;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LookupFilter {
    pub(crate) props: u32,
    pub(crate) reverse: bool,
    pub(crate) digest: Digest,
}
