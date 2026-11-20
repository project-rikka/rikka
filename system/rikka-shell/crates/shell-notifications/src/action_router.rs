use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use rikka_notifications::{CloseReason, Notification, Notifier, Posted};
use rikka_seam::{Reply, SeamClient};
use rikka_session::activation::{ActivationClient, ActivationToken};

use crate::action_request::ActionRequest;
use crate::invoker::ActionInvoker;

const MINTING: Duration = Duration::from_secs(1);

#[derive(Clone, Debug)]
pub(super) struct ActionRouter {
    notifier: Notifier,
    seam: SeamClient,
    action_requests: Rc<RefCell<HashMap<u32, HashMap<String, ActionRequest>>>>,
    invoker: ActionInvoker,
}

impl ActionRouter {
    #[must_use]
    pub(crate) fn new(notifier: Notifier, seam: SeamClient) -> ActionRouter {
        ActionRouter {
            notifier,
            seam,
            action_requests: Rc::default(),
            invoker: ActionInvoker::default(),
        }
    }

    #[must_use]
    pub(crate) fn drain(&self) -> Vec<Posted> {
        self.notifier.drain()
    }

    pub(crate) fn post(
        &self,
        notification: Notification,
        action_requests: HashMap<String, ActionRequest>,
    ) -> u32 {
        let id = self.notifier.post(notification);
        match action_requests.is_empty() {
            true => self.action_requests.borrow_mut().remove(&id),
            false => self
                .action_requests
                .borrow_mut()
                .insert(id, action_requests),
        };
        id
    }

    #[must_use]
    pub(crate) fn is_open(&self, id: u32) -> bool {
        self.notifier.is_open(id)
    }

    pub(crate) fn activate(&self, id: u32, key: &str) {
        let action_request = self
            .action_requests
            .borrow()
            .get(&id)
            .and_then(|action_requests| action_requests.get(key))
            .cloned();
        match action_request {
            Some(action_request) => self.seam.notify(action_request.request()),
            None => self.invoke(id, key),
        }
    }

    pub(crate) fn close(&self, id: u32, reason: CloseReason) {
        self.action_requests.borrow_mut().remove(&id);
        if self.invoker.defers(id, reason) {
            return;
        }
        self.notifier.close(id, reason);
    }

    fn invoke(&self, id: u32, key: &str) {
        let minting = ActivationClient::new(self.seam.clone()).mint_unattributed();
        self.invoker.begin(id);
        let (notifier, invoker, owned) =
            (self.notifier.clone(), self.invoker.clone(), key.to_owned());
        let spawned = std::thread::Builder::new()
            .name("rikka-shell-activation".to_owned())
            .spawn(move || deliver(&notifier, &invoker, id, &owned, token(minting)));
        if let Err(error) = spawned {
            tracing::debug!("running action without activation token: {error}");
            deliver(&self.notifier, &self.invoker, id, key, None);
        }
    }
}

fn deliver(
    notifier: &Notifier,
    invoker: &ActionInvoker,
    id: u32,
    key: &str,
    token: Option<String>,
) {
    notifier.activate(id, key, token.as_deref());
    if let Some(reason) = invoker.finish(id) {
        notifier.close(id, reason);
    }
}

fn token(minting: Reply) -> Option<String> {
    let Some(answer) = rikka_worker::block_on_timeout(minting, MINTING) else {
        tracing::debug!("timed out waiting for activation token");
        return None;
    };
    let minted = answer
        .inspect_err(|error| tracing::debug!("failed to get activation token: {error}"))
        .ok()?;
    minted
        .decode::<ActivationToken>()
        .ok()
        .map(|minted| minted.token)
        .filter(|token| !token.is_empty())
}
