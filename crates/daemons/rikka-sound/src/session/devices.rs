use std::cell::RefCell;
use std::rc::Weak;

use libspa::param::ParamType;
use libspa::pod::Pod;
use libspa::utils::dict::DictRef;
use pipewire::device::{Device as DeviceProxy, DeviceListener};
use pipewire::keys;
use pipewire::registry::GlobalObject;

use crate::device::Device;

use super::{Session, pods, with};

pub(super) struct WatchedDevice {
    pub(super) proxy: DeviceProxy,
    _listener: DeviceListener,
    pub(super) device: Device,
    active: Vec<(i32, i32)>,
}

impl Session {
    pub(super) fn add_device(
        &mut self,
        global: &GlobalObject<&DictRef>,
        watcher: &Weak<RefCell<Session>>,
    ) {
        let Some(props) = global.props else {
            return;
        };
        if props.get(*keys::MEDIA_CLASS) != Some("Audio/Device") {
            return;
        }
        let Ok(proxy) = self.registry.bind::<DeviceProxy, _>(global) else {
            return;
        };

        let id = global.id;
        let informed = watcher.clone();
        let parametered = watcher.clone();
        let listener = proxy
            .add_listener_local()
            .info(move |info| {
                with(&informed, |session| {
                    let Some(watched) = session.devices.get_mut(&id) else {
                        return;
                    };
                    let watched_device = watched.device.clone();
                    if let Some(props) = info.props() {
                        name_device(&mut watched.device, props);
                    }
                    let moved = watched_device != session.devices[&id].device;
                    session.publish_if(moved);
                });
            })
            .param(move |_, param, index, _, pod| {
                with(&parametered, |session| {
                    session.device_param(id, param, index, pod);
                });
            })
            .register();
        proxy.subscribe_params(&[
            ParamType::EnumProfile,
            ParamType::Profile,
            ParamType::EnumRoute,
            ParamType::Route,
        ]);

        let mut device = Device {
            id,
            name: String::new(),
            title: String::new(),
            api: None,
            profiles: Vec::new(),
            profile: None,
            routes: Vec::new(),
        };
        name_device(&mut device, props);
        self.devices.insert(
            id,
            WatchedDevice {
                proxy,
                _listener: listener,
                device,
                active: Vec::new(),
            },
        );
        self.publish();
    }

    fn device_param(&mut self, id: u32, param: ParamType, index: u32, pod: Option<&Pod>) {
        let Some(pod) = pod else {
            return;
        };
        let Some(watched) = self.devices.get_mut(&id) else {
            return;
        };
        let device = watched.device.clone();
        match param {
            ParamType::EnumProfile => {
                if index == 0 {
                    watched.device.profiles.clear();
                }
                if let Some(profile) = pods::profile(pod) {
                    watched.device.profiles.push(profile);
                }
            }
            ParamType::Profile => {
                watched.device.profile = pods::profile(pod).map(|profile| profile.index);
            }
            ParamType::EnumRoute => {
                if index == 0 {
                    watched.device.routes.clear();
                }
                if let Some(route) = pods::route(pod) {
                    watched.device.routes.push(route);
                }
            }
            ParamType::Route => {
                if index == 0 {
                    watched.active.clear();
                }
                if let Some(route) = pods::route(pod) {
                    watched.active.push((route.index, route.device));
                }
            }
            _ => return,
        }
        settle_routes(watched);
        let moved = device != self.devices[&id].device;
        self.publish_if(moved);
    }

    pub(super) fn set_device_param(&self, device: u32, param: ParamType, pod: &[u8]) {
        let (Some(watched), Some(pod)) = (self.devices.get(&device), Pod::from_bytes(pod)) else {
            return;
        };
        watched.proxy.set_param(param, 0, pod);
    }
}

fn name_device(device: &mut Device, props: &DictRef) {
    if let Some(name) = props.get(*keys::DEVICE_NAME) {
        device.name = name.to_owned();
    }
    let title = props
        .get(*keys::DEVICE_DESCRIPTION)
        .or_else(|| props.get(*keys::DEVICE_NICK));
    if let Some(title) = title {
        device.title = title.to_owned();
    }
    if device.title.is_empty() {
        device.title = device.name.clone();
    }
    if let Some(api) = props.get(*keys::DEVICE_API) {
        device.api = Some(api.to_owned());
    }
}

fn settle_routes(watched: &mut WatchedDevice) {
    let active = &watched.active;
    for route in &mut watched.device.routes {
        route.active = active
            .iter()
            .any(|(index, device)| *index == route.index && *device == route.device);
    }
}
