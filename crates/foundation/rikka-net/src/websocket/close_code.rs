use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CloseCode(u16);

impl CloseCode {
    pub const NORMAL: CloseCode = CloseCode(1000);
    pub const GOING_AWAY: CloseCode = CloseCode(1001);
    pub const PROTOCOL_ERROR: CloseCode = CloseCode(1002);
    pub const UNSUPPORTED: CloseCode = CloseCode(1003);
    pub const INVALID_DATA: CloseCode = CloseCode(1007);
    pub const POLICY: CloseCode = CloseCode(1008);
    pub const TOO_LARGE: CloseCode = CloseCode(1009);
    pub const INTERNAL_ERROR: CloseCode = CloseCode(1011);
    pub const RESTART: CloseCode = CloseCode(1012);
    pub const TRY_AGAIN_LATER: CloseCode = CloseCode(1013);

    #[must_use]
    pub const fn new(code: u16) -> CloseCode {
        CloseCode(code)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn is_application(self) -> bool {
        self.0 >= 3000
    }
}

impl From<u16> for CloseCode {
    fn from(code: u16) -> CloseCode {
        CloseCode(code)
    }
}

impl From<CloseCode> for u16 {
    fn from(code: CloseCode) -> u16 {
        code.0
    }
}

impl fmt::Display for CloseCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
