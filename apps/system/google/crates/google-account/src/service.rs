#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Service {
    Mail,
    Drive,
    Sync,
    Identity,
}

impl Service {
    pub const ALL: [Service; 4] = [
        Service::Mail,
        Service::Drive,
        Service::Sync,
        Service::Identity,
    ];

    pub(crate) const fn app(self) -> &'static str {
        match self {
            Service::Mail => "com.google.android.gm",
            Service::Drive => "com.google.android.apps.docs",
            Service::Sync => "com.android.chrome",
            Service::Identity => "com.google.android.gms",
        }
    }

    pub(crate) const fn scope(self) -> &'static str {
        match self {
            Service::Mail => "oauth2:https://mail.google.com/",
            Service::Drive => "oauth2:https://www.googleapis.com/auth/drive",
            Service::Sync => "oauth2:https://www.googleapis.com/auth/chromesync",
            Service::Identity => {
                "oauth2:openid https://www.googleapis.com/auth/userinfo.email \
                 https://www.googleapis.com/auth/userinfo.profile"
            }
        }
    }

    pub(crate) const fn slot(self) -> usize {
        match self {
            Service::Mail => 0,
            Service::Drive => 1,
            Service::Sync => 2,
            Service::Identity => 3,
        }
    }
}
