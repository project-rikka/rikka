use rikka_proton_api::{Api, Page, Paging, Session};
use rikka_proton_cache::{Cache, SINGLETON_ID, Table};
use rikka_proton_pgp::{Pgp, Secret, SecretBytes};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::account_keys::AccountKeys;
use crate::address::Address;
use crate::auth_info::{AuthInfo, AuthIntent};
use crate::challenge::Challenge;
use crate::credentialless::Credentialless;
use crate::error::{Error, Result};
use crate::fork::{FORKS_PATH, SessionFork};
use crate::fork_invitation::ForkInvitation;
use crate::forked_session::ForkedSession;
use crate::identifier::AddressId;
use crate::key_salt::KeySalt;
use crate::key_setup::KeySetup;
use crate::login::Login;
use crate::login_password_change::LoginPasswordChange;
use crate::mdm_login::MdmLogin;
use crate::modulus::SignedModulus;
use crate::passphrase;
use crate::password_change::PasswordChange;
use crate::password_scope::PasswordScope;
use crate::profile::Profile;
use crate::profile_refresh::ProfileRefresh;
use crate::public_key_lookup::PublicKeyLookup;
use crate::session_info::SessionInfo;
use crate::settings::UserSettings;
use crate::unlock_scope::UnlockScope;
use crate::user::User;

const ADDRESSES_PAGE_SIZE: u32 = 150;

const PROFILE: Table<Profile> = Table::new("account.profile");

#[derive(Debug, Clone)]
pub struct Account {
    api: Api,
    pgp: Pgp,
    cache: Option<Cache>,
}

impl Account {
    #[must_use]
    pub fn new(api: Api) -> Account {
        Account::with_pgp(api, Pgp::new())
    }

    #[must_use]
    pub fn with_pgp(api: Api, pgp: Pgp) -> Account {
        Account {
            api,
            pgp,
            cache: None,
        }
    }

    #[must_use]
    pub fn with_cache(mut self, cache: Cache) -> Account {
        self.cache = Some(cache);
        self
    }

    #[must_use]
    pub fn api(&self) -> &Api {
        &self.api
    }

    #[must_use]
    pub fn pgp(&self) -> &Pgp {
        &self.pgp
    }

    #[must_use]
    pub fn cache(&self) -> Option<&Cache> {
        self.cache.as_ref()
    }

    #[must_use]
    pub fn cached_profile(&self) -> Option<Profile> {
        self.cache.as_ref()?.get(&PROFILE, SINGLETON_ID)
    }

    pub fn cache_profile(&self, profile: &Profile) {
        if let Some(cache) = &self.cache {
            cache.put(&PROFILE, SINGLETON_ID, profile.clone());
        }
    }

    pub fn login(&self) -> Login {
        Login::new(self.clone())
    }

    pub fn fork(&self) -> SessionFork {
        SessionFork::new(self.clone())
    }

    pub fn unlock_scope(&self) -> UnlockScope {
        UnlockScope::new(self.clone())
    }

    pub fn password_scope(&self) -> PasswordScope {
        PasswordScope::new(self.clone())
    }

    pub fn credentialless(&self) -> Credentialless {
        Credentialless::new(self.clone())
    }

    pub fn mdm_login(&self, token: impl Into<String>, group: impl Into<String>) -> MdmLogin {
        MdmLogin::new(self.clone(), Secret::new(token), group.into())
    }

    pub fn setup_keys(&self) -> KeySetup {
        KeySetup::new(self.clone())
    }

    pub fn change_password<'a>(&self, keys: &'a AccountKeys) -> PasswordChange<'a> {
        PasswordChange::new(self.clone(), keys)
    }

    pub fn change_login_password(&self) -> LoginPasswordChange {
        LoginPasswordChange::new(self.clone())
    }

    pub async fn ensure_session(&self) -> Result<Session> {
        if let Some(session) = self.api.session() {
            return Ok(session);
        }

        let payload = Challenge::collect(&self.api).await.device_payload();

        Ok(self.anonymous_session(Some(payload)).await?.session())
    }

    pub async fn auth_info(&self, username: &str, intent: AuthIntent) -> Result<AuthInfo> {
        self.ensure_session().await?;

        let request = AuthInfoRequest {
            username,
            intent: intent.as_str(),
        };

        Ok(self.api.post("auth/v4/info").json(&request).send().await?)
    }

    pub async fn modulus(&self) -> Result<SignedModulus> {
        self.ensure_session().await?;

        Ok(self.api.get("auth/v4/modulus").send().await?)
    }

    pub async fn scopes(&self) -> Result<Vec<String>> {
        let response: Vec<String> = self
            .api
            .get("auth/v4/scopes")
            .send_field_or_default("Scopes")
            .await?;

        Ok(response)
    }

    pub async fn anonymous_session(&self, payload: Option<Value>) -> Result<SessionInfo> {
        let request = SessionRequest { payload };
        let session: SessionInfo = self
            .api
            .post("auth/v4/sessions")
            .json(&request)
            .anonymous()
            .send()
            .await?;

        self.api.set_session(session.session())?;

        Ok(session)
    }

    pub async fn revoke_session(&self) -> Result<()> {
        let revoked = self.api.delete("auth/v4").send_unit().await;
        self.api.clear_session()?;

        Ok(revoked?)
    }

    pub async fn fork_invitation(&self) -> Result<ForkInvitation> {
        Ok(self.api.get(FORKS_PATH).send().await?)
    }

    pub async fn forked_session(&self, selector: &str) -> Result<ForkedSession> {
        Ok(self
            .api
            .get(format!("{FORKS_PATH}/{selector}"))
            .send()
            .await?)
    }

    pub async fn lock_scopes(&self) -> Result<()> {
        self.api.put("core/v4/users/lock").send_unit().await?;

        Ok(())
    }

    pub async fn user(&self) -> Result<User> {
        Ok(self.api.get("core/v4/users").send_field("User").await?)
    }

    pub async fn settings(&self) -> Result<UserSettings> {
        let response: UserSettings = self
            .api
            .get("core/v4/settings")
            .send_field("UserSettings")
            .await?;

        Ok(response)
    }

    pub async fn key_salts(&self) -> Result<Vec<KeySalt>> {
        Ok(self
            .api
            .get("core/v4/keys/salts")
            .send_field_or_default("KeySalts")
            .await?)
    }

    pub async fn address(&self, id: &AddressId) -> Result<Address> {
        let response: Address = self
            .api
            .get(format!("core/v4/addresses/{id}"))
            .send_field("Address")
            .await?;

        Ok(response)
    }

    pub async fn addresses(&self) -> Result<Vec<Address>> {
        let mut addresses = self
            .api
            .get("core/v4/addresses")
            .pages(Paging::numbered(ADDRESSES_PAGE_SIZE))
            .collect(|response: AddressesResponse| {
                Page::new(response.addresses)
                    .total(response.total.and_then(|total| usize::try_from(total).ok()))
            })
            .await?;

        addresses.sort_by_key(Address::order);

        Ok(addresses)
    }

    pub fn public_keys<'a>(&'a self, email: &'a str) -> PublicKeyLookup<'a> {
        PublicKeyLookup::new(self, email)
    }

    pub async fn profile(&self) -> Result<Profile> {
        let (user, addresses) = futures_util::try_join!(self.user(), self.addresses())?;

        Ok(Profile::new(user, addresses))
    }

    pub async fn unlock(&self, password: impl AsRef<str>) -> Result<AccountKeys> {
        let (user, addresses, salts) =
            futures_util::join!(self.user(), self.addresses(), self.key_salts());
        let profile = Profile::new(user?, addresses?);

        let passphrase = match profile.user().keys().is_empty() {
            true => None,
            false => Some(mailbox_passphrase(profile.user(), &salts?, password.as_ref()).await?),
        };

        self.unlock_profile(profile, passphrase).await
    }

    pub async fn unlock_with_passphrase(
        &self,
        passphrase: impl AsRef<[u8]>,
    ) -> Result<AccountKeys> {
        let profile = self.profile().await?;

        self.unlock_profile(profile, Some(SecretBytes::new(passphrase.as_ref())))
            .await
    }

    pub async fn unlock_profile(
        &self,
        profile: Profile,
        passphrase: Option<SecretBytes>,
    ) -> Result<AccountKeys> {
        let keys = AccountKeys::open(self.pgp, profile, passphrase).await?;
        self.cache_profile(&keys.profile());
        Ok(keys)
    }

    pub async fn refresh(&self, keys: &AccountKeys) -> Result<ProfileRefresh> {
        let current = self.profile().await?;
        let previous = keys.profile();

        if current == previous {
            return Ok(ProfileRefresh::Unchanged);
        }
        if current.has_same_keys(&previous) {
            self.cache_profile(&current);
            return Ok(ProfileRefresh::Updated(Box::new(
                keys.with_profile(current),
            )));
        }

        let reopened = self
            .unlock_profile(current, keys.passphrase().cloned())
            .await?;
        Ok(ProfileRefresh::Rekeyed(Box::new(reopened)))
    }

    pub(crate) async fn srp_info(&self, username: &str) -> Result<AuthInfo> {
        self.auth_info(username, AuthIntent::Proton).await
    }

    pub(crate) async fn resolve_username(&self, username: Option<String>) -> Result<String> {
        if let Some(username) = username {
            return Ok(username);
        }

        let user = self.user().await?;

        user.name()
            .or_else(|| user.email())
            .map(ToOwned::to_owned)
            .ok_or(Error::MissingUsername)
    }
}

async fn mailbox_passphrase(user: &User, salts: &[KeySalt], password: &str) -> Result<SecretBytes> {
    let primary = user.primary_key().ok_or(Error::NoPrimaryKey)?;
    let salt = salts
        .iter()
        .find(|salt| salt.id() == primary.id())
        .ok_or_else(|| Error::NoKeySalt(primary.id().clone()))?
        .bytes()?;

    passphrase::derive(Secret::new(password), salt).await
}

#[derive(Debug, Serialize)]
struct AuthInfoRequest<'a> {
    #[serde(rename = "Username")]
    username: &'a str,
    #[serde(rename = "Intent")]
    intent: &'a str,
}

#[derive(Debug, Serialize)]
struct SessionRequest {
    #[serde(rename = "Payload", skip_serializing_if = "Option::is_none")]
    payload: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct AddressesResponse {
    #[serde(rename = "Addresses", default)]
    addresses: Vec<Address>,
    #[serde(rename = "Total", default)]
    total: Option<i64>,
}
