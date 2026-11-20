use rikka_proton_account::AddressId;
use rikka_proton_mail::{DirectAction, DirectSent, MailClient, MessageId, MimeType};

use crate::client::CalendarClient;
use crate::error::{Error, Result};
use crate::icalendar::{ICalendar, Method};
use crate::invite_text::ItipMail;

pub const INVITE_NAME: &str = "invite.ics";

#[must_use]
pub fn invite_mime_type(method: &Method) -> String {
    format!("text/calendar; method={}", method.as_str())
}

#[must_use]
#[derive(Debug)]
pub struct ItipSend<'a> {
    client: &'a CalendarClient,
    mail: &'a MailClient,
    from: &'a AddressId,
    calendar: &'a ICalendar,
    recipients: Vec<String>,
    content: Option<&'a ItipMail>,
    parent: Option<&'a MessageId>,
    auto_save_contacts: bool,
}

impl<'a> ItipSend<'a> {
    pub fn to(mut self, recipient: impl Into<String>) -> Self {
        self.recipients.push(recipient.into());
        self
    }

    pub fn content(mut self, content: &'a ItipMail) -> Self {
        self.content = Some(content);
        self
    }

    pub fn in_reply_to(mut self, parent: &'a MessageId) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn auto_save_contacts(mut self, save: bool) -> Self {
        self.auto_save_contacts = save;
        self
    }

    pub async fn send(self) -> Result<DirectSent> {
        if self.recipients.is_empty() {
            return Err(Error::NoRecipient);
        }
        let method = self.calendar.method().unwrap_or(Method::Publish);
        if method == Method::Reply {
            let organizer = &self.recipients[0];
            let preferences = self
                .client
                .contacts()
                .encryption_preferences(organizer, &self.mail.mail_defaults().await?)
                .await?;
            if let Some(refusal) = preferences.error {
                return Err(Error::RecipientRefused(organizer.clone(), refusal));
            }
        }

        let (subject, body) = match self.content {
            Some(content) => (content.subject.clone(), content.body.clone()),
            None => (String::new(), String::new()),
        };
        let mut sending = self
            .mail
            .direct()
            .sender(self.from)
            .subject(subject)
            .body(body)
            .mime_type(MimeType::from("text/plain"))
            .attach(
                INVITE_NAME,
                invite_mime_type(&method),
                self.calendar.to_string().into_bytes(),
            )
            .attach_public_key(false)
            .auto_save_contacts(self.auto_save_contacts && method != Method::Reply);
        for recipient in &self.recipients {
            sending = sending.to(recipient.as_str());
        }
        if let Some(parent) = self.parent {
            sending = sending.in_reply_to(parent, DirectAction::Reply);
        }
        Ok(sending.send().await?)
    }
}

impl CalendarClient {
    pub fn send_itip<'a>(
        &'a self,
        mail: &'a MailClient,
        from: &'a AddressId,
        calendar: &'a ICalendar,
    ) -> ItipSend<'a> {
        ItipSend {
            client: self,
            mail,
            from,
            calendar,
            recipients: Vec::new(),
            content: None,
            parent: None,
            auto_save_contacts: true,
        }
    }
}
