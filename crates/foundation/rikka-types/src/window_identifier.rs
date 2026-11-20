use std::fmt;
use std::str::FromStr;

const WAYLAND: &str = "wayland:";
const X11: &str = "x11:";
const WIN32: &str = "win32:";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum WindowIdentifier {
    Wayland(String),
    X11(u32),
    Win32(usize),
}

impl fmt::Display for WindowIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wayland(handle) => write!(f, "{WAYLAND}{handle}"),
            Self::X11(window) => write!(f, "{X11}{window:x}"),
            Self::Win32(window) => write!(f, "{WIN32}{window:x}"),
        }
    }
}

impl FromStr for WindowIdentifier {
    type Err = ParseWindowIdentifierError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if let Some(handle) = text.strip_prefix(WAYLAND) {
            return match handle.is_empty() || handle.chars().any(char::is_control) {
                true => Err(ParseWindowIdentifierError::Handle),
                false => Ok(Self::Wayland(handle.to_owned())),
            };
        }
        if let Some(window) = text.strip_prefix(X11) {
            return match u32::from_str_radix(hex_digits(window), 16) {
                Ok(0) | Err(_) => Err(ParseWindowIdentifierError::Window),
                Ok(window) => Ok(Self::X11(window)),
            };
        }
        if let Some(window) = text.strip_prefix(WIN32) {
            return match usize::from_str_radix(hex_digits(window), 16) {
                Ok(0) | Err(_) => Err(ParseWindowIdentifierError::Window),
                Ok(window) => Ok(Self::Win32(window)),
            };
        }
        Err(ParseWindowIdentifierError::Kind)
    }
}

fn hex_digits(text: &str) -> &str {
    text.strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseWindowIdentifierError {
    Kind,
    Handle,
    Window,
}

impl fmt::Display for ParseWindowIdentifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Kind => "unknown window identifier kind",
            Self::Handle => "invalid wayland window handle",
            Self::Window => "invalid window id",
        })
    }
}

impl std::error::Error for ParseWindowIdentifierError {}
