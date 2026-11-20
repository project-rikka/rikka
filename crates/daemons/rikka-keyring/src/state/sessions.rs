use rikka_secrets_protocol::{Algorithm, KeyExchange, Transport, WireSecret};
use zbus::zvariant::OwnedObjectPath;
use zeroize::Zeroizing;

use crate::fault::{Answer, Fault};
use crate::paths;
use crate::state::{Change, State};

#[derive(Debug)]
pub(crate) struct Session {
    owner: String,
    transport: Transport,
}

#[derive(Debug)]
pub(crate) struct NewSession {
    pub(crate) path: OwnedObjectPath,
    pub(crate) public_key: Option<Vec<u8>>,
}

impl State {
    pub(crate) fn open_session(
        &mut self,
        owner: &str,
        algorithm: Algorithm,
        peer: Option<&[u8]>,
    ) -> Answer<NewSession> {
        let (transport, public_key) = match algorithm {
            Algorithm::Plain => (Transport::Plain, None),
            Algorithm::Dh => {
                let peer =
                    peer.ok_or_else(|| Fault::invalid("the DH input is not a byte array"))?;
                let exchange = KeyExchange::new()?;
                let key = exchange.agree(peer)?;
                (Transport::Aes(key), Some(exchange.public_key().to_vec()))
            }
        };
        let serial = self.next_serial();
        let path = paths::session(serial);
        self.sessions.insert(
            path.clone(),
            Session {
                owner: owner.to_owned(),
                transport,
            },
        );
        self.changes.push(Change::SessionOpened(path.clone()));
        Ok(NewSession { path, public_key })
    }

    pub(crate) fn close_session(&mut self, path: &OwnedObjectPath, caller: &str) -> Answer<()> {
        self.transport(path, caller)?;
        self.sessions.remove(path);
        self.changes.push(Change::SessionClosed(path.clone()));
        Ok(())
    }

    pub(crate) fn client_left(&mut self, owner: &str) {
        let gone: Vec<OwnedObjectPath> = self
            .sessions
            .iter()
            .filter(|(_, session)| session.owner == owner)
            .map(|(path, _)| path.clone())
            .collect();
        for path in gone {
            self.sessions.remove(&path);
            self.changes.push(Change::SessionClosed(path));
        }
        self.dismiss_prompts_of(owner);
    }

    pub(crate) fn reveal(&self, secret: &WireSecret, caller: &str) -> Answer<Zeroizing<Vec<u8>>> {
        Ok(self
            .transport(&secret.session, caller)?
            .open(&secret.parameters, &secret.value)?)
    }

    pub(crate) fn conceal(
        &self,
        session: &OwnedObjectPath,
        caller: &str,
        plain: &[u8],
        content_type: &str,
    ) -> Answer<WireSecret> {
        let sealed = self.transport(session, caller)?.seal(plain)?;
        Ok(WireSecret {
            session: session.clone(),
            parameters: sealed.parameters,
            value: sealed.value,
            content_type: content_type.to_owned(),
        })
    }

    fn transport(&self, path: &OwnedObjectPath, caller: &str) -> Answer<&Transport> {
        let session = self
            .sessions
            .get(path)
            .ok_or_else(|| Fault::no_session(path.as_str()))?;
        if session.owner != caller {
            return Err(Fault::denied("the session belongs to another client"));
        }
        Ok(&session.transport)
    }
}
