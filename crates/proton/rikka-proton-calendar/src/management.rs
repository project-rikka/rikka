use rikka_proton_account::AddressId;
use rikka_proton_api::{base64, offload};
use rikka_proton_cache::Wired;
use rikka_proton_pgp::SessionKey;
use serde::Serialize;

use crate::bootstrap::{
    Bootstrap, CalendarKey, CalendarSettings, Passphrase, SettingsChange, UserSettings,
};
use crate::calendar::{Calendar, Member};
use crate::client::{CALENDAR_V1, CALENDAR_V2, CalendarClient};
use crate::error::{Error, Result};
use crate::identifier::{CalendarId, MemberId};
use crate::invitation::HolidayCalendar;
use crate::notification::Notification;
use crate::subscription::Subscription;
use crate::unlocked::{CalendarMaterial, UnlockedCalendar, key_setup, passphrase_signature};

const DEFAULT_COLOR: &str = "#8080FF";
const DIRECTORY_TYPE_HOLIDAYS: i32 = 2;
const VALIDATION_MODE_DOWNLOAD_AND_PARSE: i32 = 1;

#[derive(Serialize)]
struct CreationBody<'a> {
    #[serde(rename = "Name")]
    name: &'a str,
    #[serde(rename = "Description")]
    description: &'a str,
    #[serde(rename = "Color")]
    color: &'a str,
    #[serde(rename = "Display", with = "rikka_proton_api::flag")]
    display: bool,
    #[serde(rename = "AddressID")]
    address_id: &'a AddressId,
    #[serde(rename = "URL", skip_serializing_if = "Option::is_none")]
    url: Option<&'a str>,
    #[serde(rename = "IsImport", skip_serializing_if = "Option::is_none")]
    import: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MemberEdit {
    #[serde(rename = "Name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(rename = "Description", skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "Color", skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(
        rename = "Display",
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::flag::optional"
    )]
    pub display: Option<bool>,
}

#[derive(Serialize)]
struct ValidationBody<'a> {
    #[serde(rename = "URL")]
    url: &'a str,
    #[serde(rename = "Mode")]
    mode: i32,
}

#[derive(Serialize)]
struct JoinBody<'a> {
    #[serde(rename = "PassphraseKeyPacket")]
    key_packet: String,
    #[serde(rename = "Signature")]
    signature: String,
    #[serde(rename = "Color")]
    color: &'a str,
    #[serde(rename = "DefaultFullDayNotifications")]
    notifications: &'a [Notification],
    #[serde(rename = "Priority", skip_serializing_if = "Option::is_none")]
    priority: Option<i32>,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct CalendarDraft {
    client: CalendarClient,
    name: String,
    description: String,
    color: String,
    display: bool,
    address: Option<AddressId>,
    url: Option<String>,
    import: bool,
}

impl CalendarDraft {
    pub fn description(mut self, description: impl Into<String>) -> CalendarDraft {
        self.description = description.into();
        self
    }

    pub fn color(mut self, color: impl Into<String>) -> CalendarDraft {
        self.color = color.into();
        self
    }

    pub fn hidden(mut self) -> CalendarDraft {
        self.display = false;
        self
    }

    pub fn address(mut self, address: &AddressId) -> CalendarDraft {
        self.address = Some(address.clone());
        self
    }

    pub fn subscribed_to(mut self, url: impl Into<String>) -> CalendarDraft {
        self.url = Some(url.into());
        self
    }

    pub fn for_import(mut self) -> CalendarDraft {
        self.import = true;
        self
    }

    pub async fn create(self) -> Result<Calendar> {
        let address = match &self.address {
            Some(address) => address.clone(),
            None => {
                let keys = self.client.keys();
                match keys.sending_addresses().next() {
                    Some(address) => address.id().clone(),
                    None => keys.primary_address()?.id().clone(),
                }
            }
        };
        self.client.keys().signer(&address)?;

        let body = CreationBody {
            name: &self.name,
            description: &self.description,
            color: &self.color,
            display: self.display,
            address_id: &address,
            url: self.url.as_deref(),
            import: self.import.then_some(1),
        };
        let created: Calendar = self
            .client
            .api()
            .post(CALENDAR_V1)
            .json(&body)
            .send_field("Calendar")
            .await?;

        self.client.setup_keys(created.id(), &address).await?;
        Ok(created)
    }
}

impl CalendarClient {
    pub async fn calendars(&self) -> Result<Vec<Calendar>> {
        let response: Vec<Calendar> = self
            .api()
            .get(CALENDAR_V1)
            .query(&[("OrderBy", 0), ("ReturnFlags", 1)])
            .send_field_or_default("Calendars")
            .await?;
        self.cache_calendars(&response);
        Ok(response)
    }

    pub async fn calendar(&self, id: &CalendarId) -> Result<Calendar> {
        let calendar: Calendar = self
            .api()
            .get(format!("{CALENDAR_V1}/{id}"))
            .send_field("Calendar")
            .await?;
        self.cache_calendar(&calendar);
        Ok(calendar)
    }

    pub async fn bootstrap(&self, id: &CalendarId) -> Result<Bootstrap> {
        let response: Wired<Bootstrap> = self
            .api()
            .get(format!("{CALENDAR_V2}/{id}/bootstrap"))
            .send()
            .await?;
        self.cache_bootstrap(id, &response);
        Ok(response.into_value())
    }

    pub async fn calendar_keys(&self, id: &CalendarId) -> Result<Vec<CalendarKey>> {
        Ok(self
            .api()
            .get(format!("{CALENDAR_V1}/{id}/keys/all"))
            .send_field_or_default("Keys")
            .await?)
    }

    pub async fn passphrases(&self, id: &CalendarId) -> Result<Vec<Passphrase>> {
        Ok(self
            .api()
            .get(format!("{CALENDAR_V1}/{id}/passphrases"))
            .send_field_or_default("Passphrases")
            .await?)
    }

    pub async fn members(&self, id: &CalendarId) -> Result<Vec<Member>> {
        Ok(self
            .api()
            .get(format!("{CALENDAR_V1}/{id}/members/all"))
            .send_field_or_default("Members")
            .await?)
    }

    pub async fn unlock(&self, id: &CalendarId) -> Result<UnlockedCalendar> {
        let bootstrap = self.bootstrap(id).await?;
        let rotated = bootstrap
            .keys()
            .iter()
            .any(|key| key.passphrase_id() != bootstrap.passphrase().id());
        let passphrases = match rotated {
            true => self.passphrases(id).await?,
            false => vec![bootstrap.passphrase().clone()],
        };

        let pgp = *self.pgp();
        let keys = self.shared_keys();
        let id = id.clone();
        offload(move || {
            UnlockedCalendar::unlock(
                &pgp,
                &keys,
                CalendarMaterial {
                    calendar_id: &id,
                    members: bootstrap.members(),
                    keys: bootstrap.keys(),
                    passphrases: &passphrases,
                    settings: bootstrap.settings().clone(),
                },
            )
        })
        .await
    }

    pub fn new_calendar(&self, name: impl Into<String>) -> CalendarDraft {
        CalendarDraft {
            client: self.clone(),
            name: name.into(),
            description: String::new(),
            color: DEFAULT_COLOR.to_owned(),
            display: true,
            address: None,
            url: None,
            import: false,
        }
    }

    pub async fn setup_keys(&self, calendar: &CalendarId, address: &AddressId) -> Result<()> {
        let key = self.keys().signer(address)?.clone();
        let pgp = *self.pgp();
        let address = address.clone();
        let setup = offload(move || key_setup(&pgp, &address, &key)).await?;
        self.api()
            .post(format!("{CALENDAR_V1}/{calendar}/keys"))
            .json(&setup)
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn update_member(
        &self,
        calendar: &CalendarId,
        member: &MemberId,
        change: &MemberEdit,
    ) -> Result<Member> {
        Ok(self
            .api()
            .put(format!("{CALENDAR_V1}/{calendar}/members/{member}"))
            .json(change)
            .send_field("Member")
            .await?)
    }

    pub async fn delete_calendar(&self, id: &CalendarId) -> Result<()> {
        self.api()
            .delete(format!("{CALENDAR_V1}/{id}"))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn remove_holiday_calendar(&self, id: &CalendarId) -> Result<()> {
        self.api()
            .delete(format!("{CALENDAR_V1}/{id}/managed"))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn update_settings(
        &self,
        id: &CalendarId,
        change: &SettingsChange,
    ) -> Result<CalendarSettings> {
        Ok(self
            .api()
            .put(format!("{CALENDAR_V1}/{id}/settings"))
            .json(change)
            .send_field("CalendarSettings")
            .await?)
    }

    pub async fn user_settings(&self) -> Result<UserSettings> {
        let response: UserSettings = self
            .api()
            .get("settings/calendar")
            .send_field("CalendarUserSettings")
            .await?;
        self.cache_user_settings(&response);
        Ok(response)
    }

    pub async fn update_user_settings(&self, settings: &UserSettings) -> Result<UserSettings> {
        let response: UserSettings = self
            .api()
            .put("settings/calendar")
            .json(settings)
            .send_field("CalendarUserSettings")
            .await?;
        self.cache_user_settings(&response);
        Ok(response)
    }

    pub async fn subscription(&self, id: &CalendarId) -> Result<Subscription> {
        Ok(self
            .api()
            .get(format!("{CALENDAR_V1}/{id}/subscription"))
            .send_field("CalendarSubscription")
            .await?)
    }

    pub async fn validate_subscription(&self, url: &str) -> Result<()> {
        self.api()
            .post(format!("{CALENDAR_V1}/subscription/validate"))
            .json(&ValidationBody {
                url,
                mode: VALIDATION_MODE_DOWNLOAD_AND_PARSE,
            })
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn holiday_calendars(&self) -> Result<Vec<HolidayCalendar>> {
        Ok(self
            .api()
            .get(format!("{CALENDAR_V1}/directory"))
            .query(&[("Type", DIRECTORY_TYPE_HOLIDAYS)])
            .send_field_or_default("Calendars")
            .await?)
    }

    pub async fn join_holiday_calendar(
        &self,
        holiday: &HolidayCalendar,
        color: &str,
        notifications: &[Notification],
        priority: Option<i32>,
    ) -> Result<Calendar> {
        let address = self.keys().primary_address()?.id().clone();
        let key = self.keys().signer(&address)?.clone();
        let session_bytes = base64::decode(holiday.session_key_base64())
            .map_err(|_| Error::NotBase64("holiday session key"))?;
        let passphrase = holiday.passphrase().to_owned();
        let pgp = *self.pgp();

        let (key_packet, signature) = offload(move || -> Result<(String, String)> {
            let session = SessionKey::from_bytes(session_bytes)?;
            let packet = pgp.encrypt().to(key.public_key()).key_packets(&session)?;
            let signature = passphrase_signature(&pgp, passphrase.as_bytes(), &key)?;
            Ok((base64::encode(packet), signature))
        })
        .await?;

        Ok(self
            .api()
            .post(format!(
                "{CALENDAR_V1}/{}/invitations/{address}/join",
                holiday.calendar_id()
            ))
            .json(&JoinBody {
                key_packet,
                signature,
                color,
                notifications,
                priority,
            })
            .send_field("Calendar")
            .await?)
    }
}
