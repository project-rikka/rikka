use std::os::fd::OwnedFd;

use crate::common::{Server, State, Wayland, label};
use rikka_compositor_output::{LeaseConnector, LeaseDeviceId};
use rikka_wayland_client::{wp_drm_lease_connector_v1, wp_drm_lease_device_v1, wp_drm_lease_v1};
use rustix::fs::{MemfdFlags, memfd_create};

type Device = wp_drm_lease_device_v1::WpDrmLeaseDeviceV1;
type Connector = wp_drm_lease_connector_v1::WpDrmLeaseConnectorV1;
type Lease = wp_drm_lease_v1::WpDrmLeaseV1;

fn card(contents: &[u8]) -> OwnedFd {
    let fd = memfd_create("card", MemfdFlags::CLOEXEC).unwrap();
    rustix::io::write(&fd, contents).unwrap();
    fd
}

fn headset() -> LeaseConnector {
    LeaseConnector {
        id: 71,
        name: "DP-3".to_owned(),
        description: "Headset".to_owned(),
    }
}

fn add_device(server: &Server, connectors: Vec<LeaseConnector>) -> LeaseDeviceId {
    server.call(move |state: &mut State| {
        let display = state.display.clone();
        let device = state.leases.add_device(&display, card(b"card0"));
        for connector in connectors {
            state.leases.offer(device, connector);
        }
        device
    })
}

fn read_fd(client: &Wayland, owner: &str) -> Vec<u8> {
    let (_, fd) = client
        .received
        .fds
        .iter()
        .find(|(label, _)| label == owner)
        .expect("an fd arrived");
    let mut bytes = vec![0; 16];
    let read = rustix::io::pread(fd, &mut bytes, 0).unwrap();
    bytes.truncate(read);
    bytes
}

fn lease(client: &mut Wayland, device: &Device, connectors: &[&Connector]) -> Lease {
    let request = device.create_lease_request();
    for connector in connectors {
        request.request_connector(connector);
    }
    let lease = request.submit();
    client.roundtrip();
    lease
}

#[test]
fn lease_devices_are_privileged() {
    let server = Server::start();
    add_device(&server, vec![]);
    assert!(server.connect().advertises("wp_drm_lease_device_v1"));
    assert!(
        !server
            .connect_unprivileged()
            .advertises("wp_drm_lease_device_v1")
    );
}

#[test]
fn granted_lease_withdraws_connector() {
    let server = Server::start();
    let id = add_device(&server, vec![headset()]);
    let mut client = server.connect();
    let device: Device = client.global(1);
    client.roundtrip();
    let connector = client.received.connectors[0].clone();
    client.forget_events();

    let granted = lease(&mut client, &device, &[&connector]);
    assert_eq!(client.events(&granted), ["LeaseFd"]);
    assert_eq!(read_fd(&client, &label(&granted)), b"leased");
    assert_eq!(client.events(&connector), ["Withdrawn"]);
    assert_eq!(client.events(&device), ["Done"]);
    let (leased, withdrawn) = server.call(move |state| {
        (
            state.granted.clone(),
            state.leases.lease_of(id, 71).is_some(),
        )
    });
    assert_eq!(leased, [(id, vec![71])]);
    assert!(withdrawn);
    client.forget_events();

    granted.destroy();
    client.roundtrip();
    let revoked = server.call(|state| state.revoked.clone());
    assert_eq!(revoked.len(), 1);
    assert_eq!(revoked[0].0, id);
    assert_eq!(client.received.connectors.len(), 2);
    assert_eq!(kinds_of(&client, &device), ["Connector", "Done"]);
}

fn kinds_of(client: &Wayland, device: &Device) -> Vec<String> {
    client
        .events(device)
        .iter()
        .map(|event| event.split(' ').next().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn refused_lease_finishes_without_fd() {
    let server = Server::start();
    let id = add_device(&server, vec![headset()]);
    server.call(|state| state.refuse_leases = true);
    let mut client = server.connect();
    let device: Device = client.global(1);
    client.roundtrip();
    let connector = client.received.connectors[0].clone();

    let refused = lease(&mut client, &device, &[&connector]);
    assert_eq!(client.events(&refused), ["Finished"]);
    client.forget_events();

    server.call(move |state| {
        state.refuse_leases = false;
        state.leases.withdraw(id, 71);
    });
    client.roundtrip();
    assert_eq!(client.events(&connector), ["Withdrawn"]);
    let late = lease(&mut client, &device, &[&connector]);
    assert_eq!(client.events(&late), ["Finished"]);
    assert!(server.call(|state| state.granted.is_empty()));
}

#[test]
fn foreign_connector_is_protocol_error() {
    let server = Server::start();
    add_device(&server, vec![headset()]);
    add_device(&server, vec![]);
    let mut client = server.connect();
    let names = client.global_names("wp_drm_lease_device_v1");
    let _first: Device = client.bind(names[0], 1);
    let second: Device = client.bind(names[1], 1);
    client.roundtrip();
    let connector = client.received.connectors[0].clone();
    let request = second.create_lease_request();
    request.request_connector(&connector);
    assert!(client.fails());
}
