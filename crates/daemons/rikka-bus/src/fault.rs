#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Daemon {
    Accounts,
    Bluez,
    Logind,
    NetworkManager,
    Polkit,
    PowerProfiles,
    Timedated,
    UPower,
}

impl std::fmt::Display for Daemon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Daemon::Accounts => "AccountsService",
            Daemon::Bluez => "BlueZ",
            Daemon::Logind => "logind",
            Daemon::NetworkManager => "NetworkManager",
            Daemon::Polkit => "polkit",
            Daemon::PowerProfiles => "power-profiles-daemon",
            Daemon::Timedated => "timedated",
            Daemon::UPower => "UPower",
        })
    }
}

#[derive(Clone, Debug, thiserror::Error)]
pub enum Fault {
    #[error("{daemon} is unreachable: {detail}")]
    Unreachable { daemon: Daemon, detail: String },
    #[error("{daemon} denied the request: {detail}")]
    Unauthorized { daemon: Daemon, detail: String },
    #[error("{daemon} object no longer exists")]
    Gone { daemon: Daemon },
    #[error("{detail}")]
    Refused { daemon: Daemon, detail: String },
}

impl Fault {
    #[must_use]
    pub fn from_zbus(daemon: Daemon, error: &zbus::Error) -> Fault {
        let zbus::Error::MethodError(name, detail, _) = error else {
            return Fault::Unreachable {
                daemon,
                detail: error.to_string(),
            };
        };
        let detail = detail.clone().unwrap_or_else(|| name.to_string());
        Fault::named(daemon, name.as_str(), detail)
    }

    fn named(daemon: Daemon, name: &str, detail: String) -> Fault {
        match name {
            "org.freedesktop.DBus.Error.AccessDenied"
            | "org.freedesktop.DBus.Error.AuthFailed"
            | "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired" => {
                Fault::Unauthorized { daemon, detail }
            }
            "org.freedesktop.DBus.Error.UnknownObject"
            | "org.freedesktop.DBus.Error.UnknownInterface" => Fault::Gone { daemon },
            _ => Fault::Refused { daemon, detail },
        }
    }
}

pub trait DaemonError: From<Fault> + Sized {
    const DAEMON: Daemon;

    fn unreachable(detail: impl std::fmt::Display) -> Self {
        Fault::Unreachable {
            daemon: Self::DAEMON,
            detail: detail.to_string(),
        }
        .into()
    }

    fn unauthorized(detail: impl std::fmt::Display) -> Self {
        Fault::Unauthorized {
            daemon: Self::DAEMON,
            detail: detail.to_string(),
        }
        .into()
    }

    fn refused(detail: impl std::fmt::Display) -> Self {
        Fault::Refused {
            daemon: Self::DAEMON,
            detail: detail.to_string(),
        }
        .into()
    }

    fn gone() -> Self {
        Fault::Gone {
            daemon: Self::DAEMON,
        }
        .into()
    }

    fn from_standard_error_name(name: &str, detail: String) -> Self {
        Fault::named(Self::DAEMON, name, detail).into()
    }

    fn from_error_name(name: &str, detail: String) -> Self {
        Self::from_standard_error_name(name, detail)
    }

    fn from_zbus(error: &zbus::Error) -> Self {
        let zbus::Error::MethodError(name, detail, _) = error else {
            return Self::unreachable(error);
        };
        let detail = detail.clone().unwrap_or_else(|| name.to_string());
        Self::from_error_name(name.as_str(), detail)
    }
}

#[macro_export]
macro_rules! daemon_error {
    ($error:ty, $daemon:expr $(, names = $names:path)?) => {
        impl $crate::DaemonError for $error {
            const DAEMON: $crate::Daemon = $daemon;
            $(
                fn from_error_name(name: &str, detail: String) -> $error {
                    $names(name, detail)
                }
            )?
        }

        impl From<zbus::Error> for $error {
            fn from(error: zbus::Error) -> $error {
                <$error as $crate::DaemonError>::from_zbus(&error)
            }
        }

        impl From<zbus::fdo::Error> for $error {
            fn from(error: zbus::fdo::Error) -> $error {
                <$error as $crate::DaemonError>::from_zbus(&zbus::Error::from(error))
            }
        }

        impl From<zbus::zvariant::Error> for $error {
            fn from(error: zbus::zvariant::Error) -> $error {
                <$error as $crate::DaemonError>::refused(error)
            }
        }
    };
}
