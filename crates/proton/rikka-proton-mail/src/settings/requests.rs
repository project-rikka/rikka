use rikka_proton_contacts::MailDefaults;

use crate::client::MailClient;
use crate::error::Result;
use crate::settings::mail_settings::MailSettings;
use crate::settings::setting::MailSetting;

const SETTINGS: &str = "mail/v4/settings";

impl MailClient {
    pub async fn mail_settings(&self) -> Result<MailSettings> {
        let response: MailSettings = self.api().get(SETTINGS).send_field("MailSettings").await?;
        self.settings_cell().set(&response);
        Ok(response)
    }

    pub async fn cached_mail_settings(&self) -> Result<MailSettings> {
        match self.settings_cell().get() {
            Some(settings) => Ok(settings),
            None => self.mail_settings().await,
        }
    }

    pub async fn mail_defaults(&self) -> Result<MailDefaults> {
        Ok(self.cached_mail_settings().await?.mail_defaults())
    }

    pub async fn change_setting(&self, setting: &MailSetting) -> Result<()> {
        self.api()
            .put(format!("{SETTINGS}/{}", setting.path()))
            .json(&setting.body())
            .send_unit()
            .await?;
        self.settings_cell().apply(setting);
        Ok(())
    }
}
