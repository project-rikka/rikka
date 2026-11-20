use rikka_web::{PermissionDefault, SitePermission};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Permission {
    Location,
    Camera,
    Microphone,
    Notifications,
    ProtectedMedia,
    ScreenSharing,
}

impl Permission {
    pub const ALL: [Permission; 6] = [
        Permission::Location,
        Permission::Camera,
        Permission::Microphone,
        Permission::ScreenSharing,
        Permission::Notifications,
        Permission::ProtectedMedia,
    ];

    #[must_use]
    pub fn engine(self) -> Option<SitePermission> {
        Some(match self {
            Permission::Location => SitePermission::Location,
            Permission::Camera => SitePermission::Camera,
            Permission::Microphone => SitePermission::Microphone,
            Permission::Notifications => SitePermission::Notifications,
            Permission::ProtectedMedia => SitePermission::ProtectedMedia,
            Permission::ScreenSharing => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionDefaults {
    location: PermissionDefault,
    camera: PermissionDefault,
    microphone: PermissionDefault,
    notifications: PermissionDefault,
    protected_media: PermissionDefault,
    screen_sharing: PermissionDefault,
}

impl PermissionDefaults {
    #[must_use]
    pub fn get(&self, permission: Permission) -> PermissionDefault {
        match permission {
            Permission::Location => self.location,
            Permission::Camera => self.camera,
            Permission::Microphone => self.microphone,
            Permission::Notifications => self.notifications,
            Permission::ProtectedMedia => self.protected_media,
            Permission::ScreenSharing => self.screen_sharing,
        }
    }

    #[must_use]
    pub fn with(
        mut self,
        permission: Permission,
        default: PermissionDefault,
    ) -> PermissionDefaults {
        let slot = match permission {
            Permission::Location => &mut self.location,
            Permission::Camera => &mut self.camera,
            Permission::Microphone => &mut self.microphone,
            Permission::Notifications => &mut self.notifications,
            Permission::ProtectedMedia => &mut self.protected_media,
            Permission::ScreenSharing => &mut self.screen_sharing,
        };
        *slot = default;
        self
    }
}
