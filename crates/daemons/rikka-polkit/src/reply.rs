use crate::secret::Secret;

#[derive(Debug)]
pub enum Reply {
    Response { serial: u64, secret: Secret },
    Choose(usize),
    Dismiss,
}
