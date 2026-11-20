use std::sync::Arc;

use rikka_proton_account::{Account, AccountKeys, PublishedAddress};
use rikka_proton_api::{Api, Outcome, offload, parallel_map_each};
use rikka_proton_key_transparency::KeyTransparency;
use rikka_proton_pgp::{Pgp, PrivateKey, PublicKey};
use serde::Serialize;

use crate::api_keys::{ApiKeys, Recipient};
use crate::canonical::{CanonicalScheme, canonical_email};
use crate::card::{CardType, ContactCard};
use crate::contact::Contact;
use crate::contact_email::ContactEmail;
use crate::creation::ContactCreation;
use crate::decrypted_contact::DecryptedContact;
use crate::email_preferences::EmailPreferences;
use crate::encryption_preferences::{
    EncryptionPreferences, MailDefaults, SelfSend, encryption_preferences,
};
use crate::error::{Error, Result};
use crate::group::{CONTACT_GROUP, ContactGroup};
use crate::group_draft::GroupDraft;
use crate::identifier::{ContactEmailId, ContactId, GroupId};
use crate::import::parse_vcf;
use crate::kt::{KtActivation, KtAvailability, KtVerdicts};
use crate::merging::merge_vcards;
use crate::opening::decrypt_cards;
use crate::outcome::ContactReply;
use crate::pinned_keys::PinnedKeys;
use crate::queries::{ContactEmailQuery, ContactQuery, ExportQuery};
use crate::sealing::{seal_vcard, strip_trailing_spaces};
use crate::send_preferences::{MessageIntent, SendPreferences};
use crate::vcard::VCard;
use crate::verification_keys::VerificationKeys;
use crate::verifiers::{self, KnownVerifiers, Verifiers};

pub(crate) const CONTACTS: &str = "contacts/v4/contacts";
pub(crate) const LABELS: &str = "core/v4/labels";
const DELETE_BATCH: usize = 100;
const LABEL_BATCH: usize = 50;

#[derive(Clone)]
pub struct ContactsClient {
    pub(crate) account: Account,
    keys: Arc<AccountKeys>,
    transparency: Option<(KeyTransparency, KtActivation)>,
    verifiers: Arc<KnownVerifiers>,
}

impl std::fmt::Debug for ContactsClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContactsClient")
            .field("keys", &self.keys)
            .field(
                "key_transparency",
                &self.transparency.as_ref().map(|(_, activation)| activation),
            )
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
struct CardsBody {
    #[serde(rename = "Cards")]
    cards: Vec<ContactCard>,
}

#[derive(Serialize)]
struct IdsBody<'a> {
    #[serde(rename = "IDs")]
    ids: &'a [ContactId],
}

#[derive(Serialize)]
struct LabelContactsBody<'a> {
    #[serde(rename = "LabelID")]
    group: &'a GroupId,
    #[serde(rename = "ContactIDs")]
    contacts: &'a [ContactId],
}

#[derive(Serialize)]
struct LabelEmailsBody<'a> {
    #[serde(rename = "LabelID")]
    group: &'a GroupId,
    #[serde(rename = "ContactEmailIDs")]
    emails: &'a [ContactEmailId],
}

impl ContactsClient {
    #[must_use]
    pub fn new(api: Api, keys: Arc<AccountKeys>) -> ContactsClient {
        ContactsClient::with_account(Account::new(api), keys)
    }

    #[must_use]
    pub fn with_pgp(api: Api, pgp: Pgp, keys: Arc<AccountKeys>) -> ContactsClient {
        ContactsClient::with_account(Account::with_pgp(api, pgp), keys)
    }

    #[must_use]
    pub fn for_account(account: &Account, keys: &Arc<AccountKeys>) -> ContactsClient {
        ContactsClient::with_account(account.clone(), Arc::clone(keys))
    }

    fn with_account(account: Account, keys: Arc<AccountKeys>) -> ContactsClient {
        ContactsClient {
            account,
            keys,
            transparency: None,
            verifiers: Arc::default(),
        }
    }

    #[must_use]
    pub fn with_keys(mut self, keys: &Arc<AccountKeys>) -> ContactsClient {
        self.keys = Arc::clone(keys);
        self
    }

    #[must_use]
    pub fn with_key_transparency(
        mut self,
        transparency: KeyTransparency,
        activation: KtActivation,
    ) -> ContactsClient {
        self.transparency = Some((transparency, activation));
        self
    }

    #[must_use]
    pub fn key_transparency(&self) -> Option<(&KeyTransparency, KtActivation)> {
        self.transparency
            .as_ref()
            .map(|(transparency, activation)| (transparency, *activation))
    }

    #[must_use]
    pub fn account(&self) -> &Account {
        &self.account
    }

    #[must_use]
    pub fn api(&self) -> &Api {
        self.account.api()
    }

    #[must_use]
    pub fn pgp(&self) -> &Pgp {
        self.account.pgp()
    }

    #[must_use]
    pub fn keys(&self) -> &AccountKeys {
        &self.keys
    }

    pub fn contacts(&self) -> ContactQuery {
        ContactQuery::new(self.clone())
    }

    pub async fn contact(&self, id: &ContactId) -> Result<Contact> {
        let response: Contact = self
            .api()
            .get(format!("{CONTACTS}/{id}"))
            .send_field("Contact")
            .await?;
        self.cache_contacts(std::slice::from_ref(&response));
        Ok(response)
    }

    pub fn contact_emails(&self) -> ContactEmailQuery {
        ContactEmailQuery::new(self.clone())
    }

    pub async fn contact_email(&self, id: &ContactEmailId) -> Result<ContactEmail> {
        Ok(self
            .api()
            .get(format!("{CONTACTS}/emails/{id}"))
            .send_field("ContactEmail")
            .await?)
    }

    pub fn export(&self) -> ExportQuery {
        ExportQuery::new(self.clone())
    }

    pub async fn decrypt_contact(&self, contact: &Contact) -> Result<DecryptedContact> {
        if !contact.has_cards() {
            return Err(Error::NoCards(contact.id().clone()));
        }
        let client = self.clone();
        let contact = contact.clone();
        Ok(offload(move || client.decrypt_contact_blocking(&contact)).await)
    }

    pub async fn open_contact(&self, id: &ContactId) -> Result<DecryptedContact> {
        let contact = self.contact(id).await?;
        self.decrypt_contact(&contact).await
    }

    #[must_use]
    pub fn decrypt_contact_blocking(&self, contact: &Contact) -> DecryptedContact {
        decrypt_cards(self.pgp(), contact.cards(), self.keys.user_keys())
    }

    pub(crate) async fn decrypt_contacts(
        &self,
        contacts: Vec<Contact>,
    ) -> Result<Vec<DecryptedContact>> {
        let client = self.clone();
        Ok(parallel_map_each(contacts, move |contact| {
            client.decrypt_contact_blocking(&contact)
        })
        .await)
    }

    pub fn create(&self, cards: impl IntoIterator<Item = VCard>) -> ContactCreation {
        ContactCreation::new(self.clone(), cards.into_iter().collect())
    }

    pub fn import(&self, vcf: &str) -> Result<ContactCreation> {
        Ok(self
            .create(parse_vcf(vcf)?)
            .as_import(true)
            .groups_from_categories(true))
    }

    pub async fn update(&self, id: &ContactId, card: &VCard) -> Result<Contact> {
        let key = self.signing_key()?;
        let pgp = *self.pgp();
        let card = card.clone();
        let cards = offload(move || seal_vcard(&pgp, &card, &key)).await?;
        Ok(self
            .api()
            .put(format!("{CONTACTS}/{id}"))
            .json(&CardsBody { cards })
            .send_field("Contact")
            .await?)
    }

    pub async fn delete(&self, ids: &[ContactId]) -> Result<Vec<Outcome<String, ContactReply>>> {
        self.put_in_batches(format!("{CONTACTS}/delete"), ids, DELETE_BATCH, |batch| {
            IdsBody { ids: batch }
        })
        .await
    }

    pub async fn delete_all(&self) -> Result<()> {
        self.api().delete(CONTACTS).send_unit().await?;
        Ok(())
    }

    pub async fn merge(&self, ids: &[ContactId]) -> Result<Contact> {
        let [primary, others @ ..] = ids else {
            return Err(Error::NothingToMerge);
        };
        if others.is_empty() {
            return Err(Error::NothingToMerge);
        }

        let mut cards = Vec::with_capacity(ids.len());
        for id in ids {
            let contact = self.open_contact(id).await?;
            if !contact.is_complete() {
                let reason = contact
                    .problems()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ");
                return Err(Error::UnreadableContact(id.clone(), reason));
            }
            cards.push(contact.into_vcard());
        }

        let merged = offload(move || merge_vcards(&cards)).await;
        let saved = self.update(primary, &merged).await?;
        self.delete(others).await?;
        Ok(saved)
    }

    pub async fn groups(&self) -> Result<Vec<ContactGroup>> {
        let labels: Vec<ContactGroup> = self
            .api()
            .get(LABELS)
            .query(&[("Type", CONTACT_GROUP)])
            .send_field_or_default("Labels")
            .await?;
        let groups: Vec<ContactGroup> = labels
            .into_iter()
            .filter(ContactGroup::is_contact_group)
            .collect();
        self.cache_groups(&groups);
        Ok(groups)
    }

    pub fn new_group(&self, name: impl Into<String>) -> GroupDraft {
        GroupDraft::new(self.clone(), name)
    }

    pub async fn delete_group(&self, id: &GroupId) -> Result<()> {
        self.api()
            .delete(format!("{LABELS}/{id}"))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn add_contacts_to_group(
        &self,
        group: &GroupId,
        contacts: &[ContactId],
    ) -> Result<Vec<Outcome<String, ContactReply>>> {
        self.put_in_batches(
            format!("{CONTACTS}/label"),
            contacts,
            LABEL_BATCH,
            |batch| LabelContactsBody {
                group,
                contacts: batch,
            },
        )
        .await
    }

    pub async fn add_emails_to_group(
        &self,
        group: &GroupId,
        emails: &[ContactEmailId],
    ) -> Result<Vec<Outcome<String, ContactReply>>> {
        self.label_emails("label", group, emails).await
    }

    pub async fn remove_emails_from_group(
        &self,
        group: &GroupId,
        emails: &[ContactEmailId],
    ) -> Result<Vec<Outcome<String, ContactReply>>> {
        self.label_emails("unlabel", group, emails).await
    }

    async fn label_emails(
        &self,
        action: &str,
        group: &GroupId,
        emails: &[ContactEmailId],
    ) -> Result<Vec<Outcome<String, ContactReply>>> {
        self.put_in_batches(
            format!("{CONTACTS}/emails/{action}"),
            emails,
            LABEL_BATCH,
            |batch| LabelEmailsBody {
                group,
                emails: batch,
            },
        )
        .await
    }

    async fn put_in_batches<'a, T, B: Serialize>(
        &self,
        path: String,
        items: &'a [T],
        batch_size: usize,
        body: impl Fn(&'a [T]) -> B,
    ) -> Result<Vec<Outcome<String, ContactReply>>> {
        let mut outcomes = Vec::with_capacity(items.len());
        for batch in items.chunks(batch_size) {
            let response: Vec<Outcome<String, ContactReply>> = self
                .api()
                .put(path.as_str())
                .json(&body(batch))
                .send_field_or_default("Responses")
                .await?;
            outcomes.extend(response);
        }
        Ok(outcomes)
    }

    pub async fn pinned_keys(&self, email: &str) -> PinnedKeys {
        self.pinned_keys_matching(email, CanonicalScheme::Default)
            .await
    }

    async fn pinned_keys_matching(&self, email: &str, scheme: CanonicalScheme) -> PinnedKeys {
        match self.contact_pinned_keys(email, scheme).await {
            Ok(pinned) => pinned,
            Err(error) => {
                tracing::warn!(%email, %error, "failed to read pinned contact keys");
                PinnedKeys::none().with_problem(error.to_string())
            }
        }
    }

    async fn contact_pinned_keys(
        &self,
        email: &str,
        scheme: CanonicalScheme,
    ) -> Result<PinnedKeys> {
        let lookup = canonical_email(email.trim(), CanonicalScheme::Default);
        let Some(entry) = self.contact_emails().email(lookup).first().await? else {
            return Ok(PinnedKeys::none());
        };
        if entry.uses_defaults() {
            return Ok(PinnedKeys::contact_without_preferences());
        }

        let contact = self.contact(entry.contact_id()).await?;
        let Some(signed) = contact.card(CardType::Signed).cloned() else {
            return Ok(match contact.has_cards() {
                true => PinnedKeys::contact_without_preferences(),
                false => PinnedKeys::none(),
            });
        };

        let client = self.clone();
        let target = canonical_email(email.trim(), scheme);
        Ok(offload(move || {
            let status = match signed.signature() {
                Some(signature) => client
                    .pgp()
                    .verify()
                    .with_ring(client.keys.user_keys())
                    .as_utf8()
                    .detached(strip_trailing_spaces(signed.data()), signature),
                None => rikka_proton_pgp::SignatureStatus::Missing,
            };
            let verified = status.is_valid();
            let signed_at = status.created_at();
            let card = match VCard::parse(signed.data()) {
                Ok(card) => card,
                Err(error) => {
                    return PinnedKeys::contact_without_preferences()
                        .with_verification(verified, signed_at)
                        .with_problem(error.to_string());
                }
            };
            let group = card
                .emails()
                .find(|property| canonical_email(property.text_value().trim(), scheme) == target)
                .and_then(|property| property.group().map(str::to_owned));
            match group {
                Some(group) => PinnedKeys::for_contact(EmailPreferences::for_group(&card, &group))
                    .with_verification(verified, signed_at),
                None => PinnedKeys::contact_without_preferences()
                    .with_verification(verified, signed_at)
                    .with_problem("signed card has no group for email"),
            }
        })
        .await)
    }

    pub async fn encryption_preferences(
        &self,
        email: &str,
        defaults: &MailDefaults,
    ) -> Result<EncryptionPreferences> {
        let now = self.pgp().now();
        let email = email.trim().to_owned();
        if let Some(own) = self.self_send(&email) {
            return Ok(encryption_preferences(
                &email,
                &ApiKeys::own_address(),
                &PinnedKeys::none(),
                defaults,
                Some(&own),
                now,
            ));
        }

        let published = self.api_keys(&email).await?;
        let pinned = self
            .pinned_keys_matching(&email, published.canonical_scheme())
            .await;
        let defaults = *defaults;
        Ok(offload(move || {
            encryption_preferences(&email, &published, &pinned, &defaults, None, now)
        })
        .await)
    }

    pub async fn send_preferences(
        &self,
        email: &str,
        defaults: &MailDefaults,
        intent: &MessageIntent,
    ) -> Result<SendPreferences> {
        Ok(self
            .encryption_preferences(email, defaults)
            .await?
            .send_preferences(intent))
    }

    pub async fn verification_keys(&self, email: &str) -> Result<VerificationKeys> {
        let email = email.trim();
        if let Some(address) = self.keys.address_by_email(email) {
            return Ok(VerificationKeys::own(
                self.keys.verification_keys(address.id()),
            ));
        }

        let published = self.internal_api_keys(email).await?;
        let pinned = self
            .pinned_keys_matching(email, published.canonical_scheme())
            .await;
        Ok(VerificationKeys::select(
            &published,
            &pinned,
            self.kt_availability(),
        ))
    }

    pub async fn verifying_keys(&self, email: &str) -> Result<Arc<[PublicKey]>> {
        let email = email.trim().to_ascii_lowercase();
        self.verifiers
            .get_or_fetch(&email, || self.fetch_verifiers(&email))
            .await
    }

    async fn fetch_verifiers(&self, email: &str) -> Result<Verifiers> {
        if let Some(cached) = self
            .cache()
            .and_then(|cache| verifiers::cached_verifiers(cache, email))
        {
            return Ok(cached);
        }
        if let Some(address) = self.keys.address_by_email(email) {
            return Ok(self.keys.verification_keys(address.id()).into());
        }

        let lookup = self.account.public_keys(email).send().await?;
        let published: Vec<PublicKey> = match &lookup {
            PublishedAddress::Keys(keys) => keys.address().verifying_keys().collect(),
            PublishedAddress::Unpublished(_) => Vec::new(),
        };
        let internal = match lookup {
            PublishedAddress::Keys(keys) if keys.is_proton() => {
                self.evaluate_keys(email, PublishedAddress::Keys(keys))
                    .await?
            }
            _ => ApiKeys::new(Recipient::External, Vec::new()),
        };
        let pinned = self
            .pinned_keys_matching(email, internal.canonical_scheme())
            .await;
        let mut keys = VerificationKeys::select(&internal, &pinned, self.kt_availability()).keys;
        for key in published {
            if !keys
                .iter()
                .any(|pinned_key| pinned_key.fingerprint() == key.fingerprint())
            {
                keys.push(key);
            }
        }

        let keys = Verifiers::from(keys);
        if let Some(cache) = self.cache() {
            verifiers::store_verifiers(cache, email, &keys);
        }
        Ok(keys)
    }

    pub async fn api_keys(&self, email: &str) -> Result<ApiKeys> {
        let lookup = self.account.public_keys(email).send().await?;
        self.evaluate_keys(email, lookup).await
    }

    pub async fn internal_api_keys(&self, email: &str) -> Result<ApiKeys> {
        let lookup = self
            .account
            .public_keys(email)
            .internal_only()
            .send()
            .await?;
        self.evaluate_keys(email, lookup).await
    }

    async fn evaluate_keys(&self, email: &str, lookup: PublishedAddress) -> Result<ApiKeys> {
        let published = match lookup {
            PublishedAddress::Keys(published) => *published,
            unpublished if unpublished.is_external_domain() => {
                return Ok(ApiKeys::new(Recipient::External, Vec::new()));
            }
            PublishedAddress::Unpublished(refusal) => {
                return Ok(ApiKeys::new(Recipient::External, Vec::new())
                    .with_errors(vec![refusal.message.clone()]));
            }
        };

        let transparency = match self.transparency.clone() {
            Some((transparency, activation))
                if activation != KtActivation::Disabled && transparency.is_available().await =>
            {
                Some((transparency, activation))
            }
            _ => None,
        };
        let Some((transparency, activation)) = transparency else {
            return Ok(offload(move || {
                ApiKeys::from_published(&published, None, KtActivation::Disabled)
            })
            .await);
        };

        let mut check = transparency.check(email).address(published.address());
        if let Some(catch_all) = published.catch_all() {
            check = check.catch_all(catch_all);
        }
        match check.verify().await {
            Ok(verdicts) => Ok(offload(move || {
                ApiKeys::from_published(&published, Some(KtVerdicts::from(&verdicts)), activation)
            })
            .await),
            Err(rikka_proton_key_transparency::Error::Api(error)) => Err(error.into()),
            Err(error) => {
                tracing::warn!(%email, %error, "key transparency verification failed");
                Ok(offload(move || {
                    ApiKeys::from_published(&published, None, activation)
                        .failed_verification(activation)
                })
                .await)
            }
        }
    }

    fn kt_availability(&self) -> KtAvailability {
        match self.transparency {
            Some(_) => KtAvailability::Enabled,
            None => KtAvailability::Disabled,
        }
    }

    fn self_send(&self, email: &str) -> Option<SelfSend> {
        let address = self.keys.address_by_email(email.trim())?;
        let key = self
            .keys
            .signing_key(address.id())
            .map(PrivateKey::public_key);
        Some(SelfSend {
            can_receive: address.can_receive(),
            has_keys: address.has_keys() || !address.keys().is_empty(),
            key,
            can_send: address.can_send() && address.is_enabled(),
        })
    }

    pub(crate) fn signing_key(&self) -> Result<PrivateKey> {
        self.keys
            .primary_user_key()
            .cloned()
            .ok_or(Error::NoUserKey)
    }
}
