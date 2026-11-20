use rikka_proton_account::AddressId;

use crate::api::{
    BreachSummary, CustomEmailRequest, EmailBreaches, MonitorRequest, MonitorState, MonitoredEmail,
    UserMonitorRequest, VerificationCodeRequest,
};
use crate::client::{PREFIX, PassClient};
use crate::error::Result;
use crate::identifier::{CustomEmailId, ItemId, ShareId};

impl PassClient {
    pub async fn breaches(&self) -> Result<BreachSummary> {
        Ok(self
            .api()
            .get(format!("{PREFIX}/breach"))
            .send_field("Breaches")
            .await?)
    }

    pub async fn custom_emails(&self) -> Result<Vec<MonitoredEmail>> {
        let emails: Option<Vec<MonitoredEmail>> = self
            .api()
            .get(format!("{PREFIX}/breach/custom_email"))
            .send_field_at(&["Emails", "CustomEmails"])
            .await?;

        Ok(emails.unwrap_or_default())
    }

    pub async fn address_breaches(&self, address: &AddressId) -> Result<EmailBreaches> {
        self.email_breaches(format!("{PREFIX}/breach/address/{address}/breaches"))
            .await
    }

    pub async fn custom_email_breaches(&self, email: &CustomEmailId) -> Result<EmailBreaches> {
        self.email_breaches(format!("{PREFIX}/breach/custom_email/{email}/breaches"))
            .await
    }

    pub async fn alias_breaches(&self, share: &ShareId, item: &ItemId) -> Result<EmailBreaches> {
        self.email_breaches(format!("{PREFIX}/share/{share}/alias/{item}/breaches"))
            .await
    }

    pub async fn resolve_address_breaches(&self, address: &AddressId) -> Result<()> {
        self.api()
            .post(format!("{PREFIX}/breach/address/{address}/resolved"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn resolve_custom_email_breaches(&self, email: &CustomEmailId) -> Result<()> {
        self.api()
            .put(format!("{PREFIX}/breach/custom_email/{email}/resolved"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn resolve_alias_breaches(&self, share: &ShareId, item: &ItemId) -> Result<()> {
        self.api()
            .post(format!(
                "{PREFIX}/share/{share}/alias/{item}/breaches/resolved"
            ))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn add_custom_email(&self, email: &str) -> Result<MonitoredEmail> {
        let envelope: MonitoredEmail = self
            .api()
            .post(format!("{PREFIX}/breach/custom_email"))
            .json(&CustomEmailRequest { email })
            .send_field("Email")
            .await?;

        Ok(envelope)
    }

    pub async fn verify_custom_email(
        &self,
        email: &CustomEmailId,
        code: &str,
    ) -> Result<MonitoredEmail> {
        let envelope: MonitoredEmail = self
            .api()
            .put(format!("{PREFIX}/breach/custom_email/{email}/verify"))
            .json(&VerificationCodeRequest { code })
            .send_field("Email")
            .await?;

        Ok(envelope)
    }

    pub async fn resend_custom_email_verification(&self, email: &CustomEmailId) -> Result<()> {
        self.api()
            .post(format!(
                "{PREFIX}/breach/custom_email/{email}/resend_verification"
            ))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn remove_custom_email(&self, email: &CustomEmailId) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/breach/custom_email/{email}"))
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn set_custom_email_monitored(
        &self,
        email: &CustomEmailId,
        monitored: bool,
    ) -> Result<MonitoredEmail> {
        let envelope: MonitoredEmail = self
            .api()
            .put(format!("{PREFIX}/breach/custom_email/{email}/monitor"))
            .json(&MonitorRequest { monitor: monitored })
            .send_field("Email")
            .await?;

        Ok(envelope)
    }

    pub async fn set_address_monitored(&self, address: &AddressId, monitored: bool) -> Result<()> {
        self.api()
            .put(format!("{PREFIX}/breach/address/{address}/monitor"))
            .json(&MonitorRequest { monitor: monitored })
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn set_breach_monitoring(
        &self,
        proton_addresses: Option<bool>,
        aliases: Option<bool>,
    ) -> Result<MonitorState> {
        let envelope: Option<MonitorState> = self
            .api()
            .put(format!("{PREFIX}/user/monitor"))
            .json(&UserMonitorRequest {
                proton_addresses,
                aliases,
            })
            .send_field_or_default("Monitor")
            .await?;

        match envelope {
            Some(monitor) => Ok(monitor),
            None => Ok(self.access().await?.monitor()),
        }
    }

    async fn email_breaches(&self, path: String) -> Result<EmailBreaches> {
        let envelope: EmailBreaches = self.api().get(path).send_field("Breaches").await?;

        Ok(envelope)
    }
}
