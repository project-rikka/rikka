use std::fmt;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixListener;

use rikka_compositor_protocols::wp_security_context_manager_v1::{
    self, Error as ManagerError, WpSecurityContextManagerV1,
};
use rikka_compositor_protocols::wp_security_context_v1::{self, Error, WpSecurityContextV1};
use rikka_compositor_reactor::{Interest, Reactor, Token};
use rikka_compositor_server::{
    Client, ClientOptions, DisplayHandle, Listener, Module, SecurityContext,
};
use slotmap::{SlotMap, new_key_type};

use crate::state::{Compositor, Core};

new_key_type! {
    struct DraftId;
}

rikka_compositor_server::slot_keys!(DraftId);

pub(crate) struct SandboxProtocol;

impl Module<Compositor> for SandboxProtocol {
    fn install(self, display: &DisplayHandle<Compositor>) {
        display.serve::<WpSecurityContextManagerV1>(manager);
        display
            .serve::<WpSecurityContextV1>(context)
            .destroyed(context_destroyed);
        display
            .global::<WpSecurityContextManagerV1>()
            .privileged()
            .publish();
    }
}

#[derive(Default)]
pub(crate) struct Sandboxes {
    open: Vec<Sandbox>,
    drafts: SlotMap<DraftId, Draft>,
    minted: u64,
}

struct Sandbox {
    id: u64,
    token: Token,
    _close: OwnedFd,
    _listener: Listener,
}

struct Draft {
    listen: Option<OwnedFd>,
    close: Option<OwnedFd>,
    context: SecurityContext,
}

fn manager(
    compositor: &mut Compositor,
    manager: WpSecurityContextManagerV1,
    request: wp_security_context_manager_v1::Request,
) {
    let wp_security_context_manager_v1::Request::CreateListener {
        id,
        listen_fd,
        close_fd,
    } = request
    else {
        return;
    };
    if Client::from_resource(&manager).is_some_and(|client| client.security_context().is_some()) {
        return manager.post_error(ManagerError::Nested);
    }
    if !rustix::net::sockopt::socket_acceptconn(&listen_fd).unwrap_or(false) {
        return manager.post_error(ManagerError::InvalidListenFd);
    }
    let draft = compositor.sandboxes.drafts.insert(Draft {
        listen: Some(listen_fd),
        close: Some(close_fd),
        context: SecurityContext::default(),
    });
    id.set_key(draft);
}

fn context(
    compositor: &mut Compositor,
    context: WpSecurityContextV1,
    request: wp_security_context_v1::Request,
) {
    if matches!(request, wp_security_context_v1::Request::Destroy) {
        return;
    }
    let Some(draft) = context
        .key::<DraftId>()
        .and_then(|id| compositor.sandboxes.drafts.get_mut(id))
    else {
        return context.post_error(Error::AlreadyUsed);
    };
    if draft.listen.is_none() {
        return context.post_error(Error::AlreadyUsed);
    }
    match request {
        wp_security_context_v1::Request::Destroy => {}
        wp_security_context_v1::Request::SetSandboxEngine { name } => {
            describe(&context, &mut draft.context.sandbox_engine, name);
        }
        wp_security_context_v1::Request::SetAppId { app_id } => {
            describe(&context, &mut draft.context.app_id, app_id);
        }
        wp_security_context_v1::Request::SetInstanceId { instance_id } => {
            describe(&context, &mut draft.context.instance_id, instance_id);
        }
        wp_security_context_v1::Request::Commit => {
            if let (Some(listen), Some(close)) = (draft.listen.take(), draft.close.take()) {
                let sandbox = draft.context.clone();
                let Compositor {
                    sandboxes, core, ..
                } = compositor;
                sandboxes.open(core, UnixListener::from(listen), close, sandbox);
            }
        }
    }
}

fn context_destroyed(compositor: &mut Compositor, context: WpSecurityContextV1) {
    if let Some(id) = context.key::<DraftId>() {
        compositor.sandboxes.drafts.remove(id);
    }
}

fn describe(context: &WpSecurityContextV1, slot: &mut Option<String>, value: &str) {
    if slot.is_some() {
        context.post_error(Error::AlreadySet);
    } else {
        *slot = Some(value.to_owned());
    }
}

impl Sandboxes {
    fn open(
        &mut self,
        core: &Core,
        listen: UnixListener,
        close: OwnedFd,
        sandbox: SecurityContext,
    ) {
        let engine = sandbox.sandbox_engine.clone();
        let listener = match core
            .display
            .accept_on(listen, ClientOptions::new().security_context(sandbox))
        {
            Ok(listener) => listener,
            Err(error) => {
                return tracing::warn!("cannot listen on sandbox socket: {error}");
            }
        };
        self.minted += 1;
        let id = self.minted;
        let watched = core.reactor.watch(
            close.as_fd(),
            Interest::READABLE,
            move |compositor: &mut Compositor, _| {
                compositor.sandboxes.close(&compositor.core.reactor, id)
            },
        );
        match watched {
            Ok(token) => {
                tracing::debug!(?engine, "sandbox socket accepting clients");
                self.open.push(Sandbox {
                    id,
                    token,
                    _close: close,
                    _listener: listener,
                });
            }
            Err(error) => tracing::warn!("cannot watch sandbox close fd: {error}"),
        }
    }

    fn close(&mut self, reactor: &Reactor<Compositor>, id: u64) {
        let Some(at) = self.open.iter().position(|sandbox| sandbox.id == id) else {
            return;
        };
        let sandbox = self.open.swap_remove(at);
        reactor.remove(sandbox.token);
        tracing::debug!("sandbox socket closed");
    }
}

impl fmt::Debug for Sandboxes {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Sandboxes")
            .field("open", &self.open.len())
            .finish_non_exhaustive()
    }
}
