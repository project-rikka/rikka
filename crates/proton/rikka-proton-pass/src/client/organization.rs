use rikka_proton_api::{Page, Paging};

use crate::api::{
    MemberReport, MonitorReport, Organization, OrganizationUpdate, PasswordPolicy, PauseUrlRequest,
    PauseValues, PausedUrl, PausedUrlPage, ReportPage,
};
use crate::client::{PAGE_SIZE, PREFIX, PassClient};
use crate::error::Result;
use crate::identifier::PausedUrlId;

impl PassClient {
    pub async fn organization(&self) -> Result<Option<Organization>> {
        let envelope: Option<Organization> = self
            .api()
            .get(format!("{PREFIX}/organization"))
            .send_field_or_default("Organization")
            .await?;

        Ok(envelope)
    }

    pub async fn update_organization(
        &self,
        update: &OrganizationUpdate,
    ) -> Result<Option<Organization>> {
        let envelope: Option<Organization> = self
            .api()
            .put(format!("{PREFIX}/organization/settings"))
            .json(update)
            .send_field_or_default("Organization")
            .await?;

        Ok(envelope)
    }

    pub async fn update_password_policy(
        &self,
        policy: &PasswordPolicy,
    ) -> Result<Option<Organization>> {
        let envelope: Option<Organization> = self
            .api()
            .put(format!("{PREFIX}/organization/settings/password_policy"))
            .json(policy)
            .send_field_or_default("Organization")
            .await?;

        Ok(envelope)
    }

    pub async fn organization_reports(&self) -> Result<Vec<MemberReport>> {
        Ok(self
            .api()
            .get(format!("{PREFIX}/organization/report"))
            .pages(Paging::numbered(PAGE_SIZE))
            .field("Report")
            .collect(|page: ReportPage| Page::new(page.members).total(page.total))
            .await?)
    }

    pub async fn send_monitor_report(&self, report: &MonitorReport) -> Result<()> {
        self.api()
            .post(format!("{PREFIX}/organization/report/client_data"))
            .json(report)
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn paused_urls(&self) -> Result<Vec<PausedUrl>> {
        Ok(self
            .api()
            .get(format!("{PREFIX}/organization/urlpause"))
            .pages(Paging::since())
            .field("Entries")
            .collect(|page: PausedUrlPage| Page::new(page.entries).next(page.last_id))
            .await?)
    }

    pub async fn pause_url(&self, url: &str, values: PauseValues) -> Result<PausedUrl> {
        let envelope: PausedUrl = self
            .api()
            .post(format!("{PREFIX}/organization/urlpause"))
            .json(&PauseUrlRequest { url, values })
            .send_field("Entry")
            .await?;

        Ok(envelope)
    }

    pub async fn update_paused_url(&self, entry: &PausedUrlId, values: PauseValues) -> Result<()> {
        self.api()
            .put(format!("{PREFIX}/organization/urlpause/{entry}"))
            .json(&values)
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn remove_paused_url(&self, entry: &PausedUrlId) -> Result<()> {
        self.api()
            .delete(format!("{PREFIX}/organization/urlpause/{entry}"))
            .send_unit()
            .await?;

        Ok(())
    }
}
