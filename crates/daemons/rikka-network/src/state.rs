use futures_util::Stream;

use crate::access_point::AccessPoint;
use crate::device::{Device, DeviceId, DeviceKind};
use crate::event::Event;
use crate::invitation::Invitation;
use crate::joining::{Joining, Unjoinable};
use crate::profile::Profile;
use crate::request::{Action, Join, JoinTarget, Request};
use crate::session::Nm;
use crate::snapshot::Snapshot;
use rikka_bus::{Apply, Monitor, Refused, Refusing, Service};

#[derive(Clone, Debug, Default)]
pub struct State {
    pub snapshot: Snapshot,
    pub error: Option<Refused<Action>>,
}

impl State {
    #[must_use]
    pub fn radio(&self) -> Option<&Device> {
        self.snapshot.devices_of(DeviceKind::Wireless).next()
    }

    #[must_use]
    pub fn networks(&self) -> Vec<&AccessPoint> {
        self.radio()
            .map(|device| self.snapshot.networks(&device.id))
            .unwrap_or_default()
    }

    #[must_use]
    pub fn joined(&self) -> Option<&AccessPoint> {
        let device = &self.radio()?.id;
        self.snapshot
            .access_points
            .iter()
            .find(|point| &point.device == device && point.in_use)
    }

    #[must_use]
    pub fn is_wireless_on(&self) -> bool {
        self.snapshot.networking && self.snapshot.wireless.enabled
    }

    #[must_use]
    pub fn is_killed(&self) -> bool {
        !self.snapshot.wireless.hardware
    }
}

impl Refusing for State {
    type Action = Action;

    fn refused(&mut self) -> &mut Option<Refused<Action>> {
        &mut self.error
    }
}

impl Apply<Event> for State {
    fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Changed(snapshot) => self.snapshot.apply(*snapshot),
            Event::Answered { action, error } => self.answer(action, error),
            Event::Revealed { .. } => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Network(Monitor<Service<Nm>>);

impl Network {
    #[must_use]
    pub fn start() -> Network {
        Network(Monitor::start())
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.0.state()
    }

    pub fn changes(&self) -> impl Stream<Item = State> + Send + 'static {
        self.0.changes()
    }

    pub fn events(&self) -> impl Stream<Item = Event> + Send + 'static {
        self.0.events()
    }

    #[must_use]
    pub fn unavailable(&self) -> Option<String> {
        self.0.unavailable()
    }

    pub fn unavailable_changes(&self) -> impl Stream<Item = Option<String>> + Send + 'static {
        self.0.unavailable_changes()
    }

    pub fn look(&self) {
        if let Some(device) = self.radio() {
            self.send(Request::Scan(device));
        }
    }

    pub fn use_wireless(&self, on: bool) {
        self.send(Request::Radio(on));
    }

    pub fn join(&self, point: &AccessPoint, passphrase: String) {
        let request = match &point.saved {
            Some(uuid) => Request::Activate {
                uuid: uuid.clone(),
                device: Some(point.device.clone()),
            },
            None => Request::Join(Box::new(Join {
                device: point.device.clone(),
                network: JoinTarget::Visible(point.id.clone()),
                security: point.security,
                passphrase,
                account: None,
                shared: false,
            })),
        };
        self.send(request);
    }

    pub fn accept(&self, invitation: &Invitation) -> Result<Joining, Unjoinable> {
        let state = self.state();
        let device = state.radio().ok_or(Unjoinable::NoRadio)?.id.clone();
        if !state.is_wireless_on() {
            return Err(Unjoinable::RadioOff);
        }
        if invitation.security.needs_account() {
            return Err(Unjoinable::NeedsAccount);
        }
        let joining = Joining::new(invitation.ssid.clone(), device.clone());
        let point = state
            .networks()
            .into_iter()
            .find(|point| point.ssid == invitation.ssid);
        let (network, security) = match point {
            Some(point) if point.in_use => return Ok(joining),
            Some(point) if point.security.needs_account() => return Err(Unjoinable::NeedsAccount),
            Some(point) if point.saved.is_some() => {
                if let Some(renewed) = renewed(&state, point, invitation) {
                    self.send(Request::Save(Box::new(renewed)));
                }
                self.join(point, String::new());
                return Ok(joining);
            }
            Some(point) => (JoinTarget::Visible(point.id.clone()), point.security),
            None if invitation.hidden => (
                JoinTarget::Hidden(invitation.ssid.clone()),
                invitation.security,
            ),
            None => return Err(Unjoinable::OutOfRange),
        };
        self.send(Request::Join(Box::new(Join {
            device,
            network,
            security,
            passphrase: invitation.passphrase.clone(),
            account: None,
            shared: false,
        })));
        Ok(joining)
    }

    pub fn leave(&self) {
        if let Some(device) = self.radio() {
            self.send(Request::Disconnect(device));
        }
    }

    fn radio(&self) -> Option<DeviceId> {
        self.0
            .read(|state| state.radio().map(|device| device.id.clone()))
    }

    pub fn send(&self, request: Request) {
        self.0.attempt(request);
    }
}

fn renewed(state: &State, point: &AccessPoint, invitation: &Invitation) -> Option<Profile> {
    if invitation.passphrase.is_empty() || !point.security.needs_passphrase() {
        return None;
    }
    let mut profile = state.snapshot.profile(point.saved.as_deref()?)?.clone();
    profile.security.as_mut()?.passphrase = Some(invitation.passphrase.clone());
    Some(profile)
}
