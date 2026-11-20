use proton_session::Failure;
use rikka_proton_authenticator::AuthenticatorClient;
use rikka_proton_client::{EventLoop, Session};
use rikka_proton_contacts::{ContactsClient, KtActivation};
use rikka_proton_drive::DriveClient;
use rikka_proton_key_transparency::KeyTransparency;
use rikka_proton_mail::MailClient;
use rikka_proton_pass::{PassClient, UserEvents};

#[derive(Debug)]
pub(crate) enum Update {
    Mail(Box<rikka_proton_mail::Event>),
    Pass(Box<UserEvents>),
}

#[derive(Debug, Clone)]
pub(crate) struct Clients {
    session: Session,
    mail: MailClient,
    drive: DriveClient,
    pass: Result<PassClient, Failure>,
    codes: Result<AuthenticatorClient, Failure>,
}

impl Clients {
    pub(crate) fn new(session: Session) -> Clients {
        let proton = session.proton();
        let (account, keys) = (proton.account(), session.keys());
        let transparency = KeyTransparency::for_account(account, proton.log_lists().clone());
        let contacts = ContactsClient::for_account(account, keys)
            .with_key_transparency(transparency, KtActivation::default());
        Clients {
            mail: MailClient::for_account(account, keys).with_contacts(contacts),
            drive: DriveClient::for_account(account, keys),
            pass: PassClient::for_account(account, keys).map_err(Failure::from),
            codes: AuthenticatorClient::for_account(account, keys).map_err(Failure::from),
            session,
        }
    }

    pub(crate) fn renewed(&self, session: Session) -> Clients {
        let keys = session.keys();
        Clients {
            mail: self.mail.clone().with_keys(keys),
            drive: self.drive.clone().with_keys(keys),
            pass: self
                .pass
                .clone()
                .and_then(|pass| pass.with_keys(keys).map_err(Failure::from)),
            codes: self.codes.clone(),
            session,
        }
    }

    pub(crate) fn session(&self) -> &Session {
        &self.session
    }

    pub(crate) fn mail(&self) -> MailClient {
        self.mail.clone()
    }

    pub(crate) fn drive(&self) -> DriveClient {
        self.drive.clone()
    }

    pub(crate) fn pass(&self) -> Result<PassClient, Failure> {
        self.pass.clone()
    }

    pub(crate) fn codes(&self) -> Result<AuthenticatorClient, Failure> {
        self.codes.clone()
    }

    pub(crate) fn event_loop(&self) -> EventLoop<Update> {
        let events = self.session.event_loop().feed(self.mail(), Update::Mail);
        match self.pass() {
            Ok(pass) => events.feed(pass, Update::Pass),
            Err(_) => events,
        }
    }
}
