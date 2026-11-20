use rikka_proton_client::Session;

#[derive(Debug)]
pub enum Renewed {
    Current,
    ProfileChanged(Box<Session>),
    KeysChanged(Box<Session>),
    SignedOut,
}
