use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rikka_remote_link::{ServerIdentity, server_endpoint};
use rikka_remote_wire::EndReason;
use rikka_session::remote::{Address, Reach, Serving};
use tokio::sync::{oneshot, watch};
use tokio::task::AbortHandle;

use crate::access::Access;
use crate::error::{Error, Result};
use crate::identity::Identity;
use crate::notices::Notices;
use crate::options::{Listening, MediaOptions, Options};
use crate::status::StatusSink;
use crate::viewer;

const TAILSCALE_INTERFACE: &str = "tailscale";
const TAILNET_V4: (Ipv4Addr, u32) = (Ipv4Addr::new(100, 64, 0, 0), 10);
const TAILNET_V6: (Ipv6Addr, u32) = (Ipv6Addr::new(0xfd7a, 0x115c, 0xa1e0, 0, 0, 0, 0, 0), 48);
const STOP_PATIENCE: Duration = Duration::from_secs(3);

struct Live {
    id: u64,
    end: Option<oneshot::Sender<EndReason>>,
    done: oneshot::Receiver<()>,
}

pub(crate) struct Claim {
    pub(crate) id: u64,
    pub(crate) end: oneshot::Receiver<EndReason>,
    pub(crate) _done: oneshot::Sender<()>,
}

pub(crate) struct Hub {
    pub(crate) identity: Identity,
    pub(crate) access: Access,
    pub(crate) notices: Notices,
    pub(crate) status: StatusSink,
    options: watch::Sender<MediaOptions>,
    live: tokio::sync::Mutex<Option<Live>>,
    next: AtomicU64,
}

impl Hub {
    pub(crate) fn options(&self) -> watch::Receiver<MediaOptions> {
        self.options.subscribe()
    }

    pub(crate) async fn claim(&self) -> Claim {
        let mut live = self.live.lock().await;
        if let Some(previous) = live.take() {
            previous.finish(EndReason::Replaced).await;
        }
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (end, ending) = oneshot::channel();
        let (done, waited) = oneshot::channel();
        *live = Some(Live {
            id,
            end: Some(end),
            done: waited,
        });
        Claim {
            id,
            end: ending,
            _done: done,
        }
    }

    pub(crate) async fn release(&self, id: u64) {
        let mut live = self.live.lock().await;
        if live.as_ref().is_some_and(|live| live.id == id) {
            *live = None;
        }
    }

    pub(crate) async fn end_live(&self, reason: EndReason) {
        let mut live = self.live.lock().await;
        if let Some(live) = live.as_mut() {
            live.end(reason);
        }
    }

    async fn retire(&self, reason: EndReason) {
        let previous = self.live.lock().await.take();
        if let Some(previous) = previous {
            previous.finish(reason).await;
        }
    }
}

impl Live {
    fn end(&mut self, reason: EndReason) {
        if let Some(end) = self.end.take()
            && end.send(reason).is_err()
        {
            tracing::debug!("the viewer had already left");
        }
    }

    async fn finish(mut self, reason: EndReason) {
        self.end(reason);
        self.done.await.unwrap_or(());
    }
}

pub(crate) struct Server {
    hub: Arc<Hub>,
    endpoint: quinn::Endpoint,
    accepting: AbortHandle,
    serving: Serving,
    runtime: tokio::runtime::Handle,
}

impl Server {
    pub(crate) fn start(
        options: &Options,
        runtime: &tokio::runtime::Handle,
        status: StatusSink,
    ) -> Result<Server> {
        let identity = Identity::current()?;
        let certificate = ServerIdentity::load_or_create(&identity.certificate(), &identity.host)
            .map_err(Error::Identity)?;
        let port = options.listening.port;
        let endpoint = {
            let _entered = runtime.enter();
            server_endpoint(&certificate, port).map_err(|error| Error::Listen { port, error })?
        };
        let notices = Notices::new(runtime.clone());
        let hub = Arc::new(Hub {
            access: Access::load(&identity, notices.clone()),
            identity,
            notices,
            status,
            options: watch::Sender::new(options.media.clone()),
            live: tokio::sync::Mutex::new(None),
            next: AtomicU64::new(1),
        });
        let listening = options.listening.clone();
        let accepting = runtime
            .spawn(accept(endpoint.clone(), Arc::clone(&hub), listening))
            .abort_handle();
        tracing::info!(port, fingerprint = %certificate.pin(), "listening for remote viewers");
        Ok(Server {
            serving: Serving {
                port,
                addresses: addresses(port, &options.listening),
                fingerprint: certificate.pin().to_string(),
            },
            hub,
            endpoint,
            accepting,
            runtime: runtime.clone(),
        })
    }

    pub(crate) fn serving(&self) -> Serving {
        self.serving.clone()
    }

    pub(crate) fn set_media(&self, media: MediaOptions) {
        self.hub.options.send_replace(media);
    }

    pub(crate) fn disconnect(&self) {
        let hub = Arc::clone(&self.hub);
        self.runtime
            .spawn(async move { hub.end_live(EndReason::Disconnected).await });
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.accepting.abort();
        let hub = Arc::clone(&self.hub);
        let retired = self.runtime.block_on(async move {
            tokio::time::timeout(STOP_PATIENCE, hub.retire(EndReason::Stopped)).await
        });
        if retired.is_err() {
            tracing::warn!("the viewer did not leave in time");
        }
        self.endpoint.close(0_u32.into(), b"stopped");
        let endpoint = self.endpoint.clone();
        self.runtime
            .spawn(async move { endpoint.wait_idle().await });
    }
}

async fn accept(endpoint: quinn::Endpoint, hub: Arc<Hub>, listening: Listening) {
    while let Some(incoming) = endpoint.accept().await {
        let from = incoming.remote_address();
        if !admits(reach_of(from.ip(), incoming.local_ip()), &listening) {
            tracing::info!(%from, "refused a viewer outside the allowed networks");
            incoming.refuse();
            continue;
        }
        tokio::spawn(viewer::serve(Arc::clone(&hub), incoming));
    }
}

fn admits(reach: Reach, listening: &Listening) -> bool {
    match reach {
        Reach::Local => true,
        Reach::Tailnet => listening.tailscale || listening.network,
        Reach::Network => listening.network,
    }
}

fn reach_of(from: IpAddr, local: Option<IpAddr>) -> Reach {
    let from = from.to_canonical();
    if from.is_loopback() {
        return Reach::Local;
    }
    match (
        is_tailnet(from),
        local.map(|local| is_tailnet(local.to_canonical())),
    ) {
        (true, Some(true)) => Reach::Tailnet,
        _ => Reach::Network,
    }
}

fn is_tailnet(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => inside_v4(address, TAILNET_V4),
        IpAddr::V6(address) => inside_v6(address, TAILNET_V6),
    }
}

fn inside_v4(address: Ipv4Addr, (network, bits): (Ipv4Addr, u32)) -> bool {
    let mask = u32::MAX << (32 - bits);
    u32::from(address) & mask == u32::from(network) & mask
}

fn inside_v6(address: Ipv6Addr, (network, bits): (Ipv6Addr, u32)) -> bool {
    let mask = u128::MAX << (128 - bits);
    u128::from(address) & mask == u128::from(network) & mask
}

fn addresses(port: u16, listening: &Listening) -> Vec<Address> {
    let mut found = vec![Address {
        address: format!("localhost:{port}"),
        reach: Reach::Local,
    }];
    let interfaces = rikka_interfaces::interface_addresses().unwrap_or_else(|error| {
        tracing::warn!("cannot list network interfaces: {error}");
        Vec::new()
    });
    for interface in &interfaces {
        let IpAddr::V4(address) = interface.address else {
            continue;
        };
        if address.is_loopback() {
            continue;
        }
        let reach = match interface.name.starts_with(TAILSCALE_INTERFACE)
            || inside_v4(address, TAILNET_V4)
        {
            true => Reach::Tailnet,
            false => Reach::Network,
        };
        if admits(reach, listening) {
            found.push(Address {
                address: SocketAddr::new(IpAddr::V4(address), port).to_string(),
                reach,
            });
        }
    }
    found
}
