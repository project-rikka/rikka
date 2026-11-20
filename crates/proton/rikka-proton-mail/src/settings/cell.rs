use rikka_sync::Mutex;

use crate::settings::mail_settings::MailSettings;
use crate::settings::setting::MailSetting;

#[derive(Debug, Default)]
pub(crate) struct SettingsCell {
    current: Mutex<Option<MailSettings>>,
}

impl SettingsCell {
    pub(crate) fn get(&self) -> Option<MailSettings> {
        self.current.lock().clone()
    }

    pub(crate) fn set(&self, settings: &MailSettings) {
        *self.current.lock() = Some(settings.clone());
    }

    pub(crate) fn apply(&self, setting: &MailSetting) {
        if let Some(current) = self.current.lock().as_mut() {
            current.apply(setting);
        }
    }
}
