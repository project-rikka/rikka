#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpacePrivacy {
    pub directs: bool,
    pub message_requests: bool,
    pub activity: bool,
    pub joining: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivacyChange {
    Directs(bool),
    MessageRequests(bool),
    Activity(bool),
    Joining(bool),
}
