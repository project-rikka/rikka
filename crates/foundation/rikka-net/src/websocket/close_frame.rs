use tokio_tungstenite::tungstenite::protocol::CloseFrame as TungsteniteFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode as TungsteniteCode;

use crate::websocket::close_code::CloseCode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloseFrame {
    pub code: CloseCode,
    pub reason: String,
}

impl CloseFrame {
    #[must_use]
    pub fn new(code: CloseCode, reason: impl Into<String>) -> CloseFrame {
        CloseFrame {
            code,
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn normal() -> CloseFrame {
        CloseFrame::new(CloseCode::NORMAL, "")
    }
}

impl Default for CloseFrame {
    fn default() -> CloseFrame {
        CloseFrame::normal()
    }
}

impl CloseFrame {
    pub(crate) fn from_tungstenite(frame: TungsteniteFrame) -> CloseFrame {
        CloseFrame {
            code: CloseCode::new(frame.code.into()),
            reason: frame.reason.as_str().to_owned(),
        }
    }

    pub(crate) fn into_tungstenite(self) -> TungsteniteFrame {
        TungsteniteFrame {
            code: TungsteniteCode::from(self.code.get()),
            reason: self.reason.into(),
        }
    }
}
