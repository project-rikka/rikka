use std::collections::HashMap;

use futures_util::StreamExt;
use rikka_network::{Network, State};
use rikka_test_bus::{PrivateBus, TIMEOUT};
use rikka_test_objects::FakeService;
use tokio::runtime::Runtime;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

const SERVICE: &str = "org.freedesktop.NetworkManager";
const ROOT: &str = "/org/freedesktop";
const MANAGER_PATH: &str = "/org/freedesktop/NetworkManager";
const MANAGER: &str = "org.freedesktop.NetworkManager";
const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const SETTINGS: &str = "org.freedesktop.NetworkManager.Settings";
const CONNECTION: &str = "org.freedesktop.NetworkManager.Settings.Connection";
const DEVICE_PATH: &str = "/org/freedesktop/NetworkManager/Devices/1";
const DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const WIRELESS: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const POINT_PATH: &str = "/org/freedesktop/NetworkManager/AccessPoint/1";
const ACCESS_POINT: &str = "org.freedesktop.NetworkManager.AccessPoint";
const HOME: &str = "/org/freedesktop/NetworkManager/Settings/1";
const OFFICE: &str = "/org/freedesktop/NetworkManager/Settings/2";
const DEVICE_WIFI: u32 = 2;
const DEVICE_ACTIVATED: u32 = 100;

type Settings = HashMap<String, HashMap<String, OwnedValue>>;

fn owned(value: Value<'_>) -> OwnedValue {
    OwnedValue::try_from(value).expect("settings hold no file descriptors")
}

fn saved(uuid: &str, id: &str, ssid: &str) -> Settings {
    HashMap::from([
        (
            "connection".to_owned(),
            HashMap::from([
                ("uuid".to_owned(), owned(Value::from(uuid))),
                ("id".to_owned(), owned(Value::from(id))),
                ("type".to_owned(), owned(Value::from("802-11-wireless"))),
            ]),
        ),
        (
            "802-11-wireless".to_owned(),
            HashMap::from([(
                "ssid".to_owned(),
                owned(Value::from(ssid.as_bytes().to_vec())),
            )]),
        ),
    ])
}

fn network_manager() -> FakeService {
    let service = FakeService::serve(SERVICE, ROOT);
    service.add(
        MANAGER_PATH,
        MANAGER,
        [
            ("NetworkingEnabled", Value::from(true)),
            ("WirelessEnabled", Value::from(true)),
            ("WirelessHardwareEnabled", Value::from(true)),
        ],
    );
    service.add(
        SETTINGS_PATH,
        SETTINGS,
        [("Hostname", Value::from("rikka"))],
    );
    service.answer(
        HOME,
        CONNECTION,
        "GetSettings",
        saved("home", "Home", "Home"),
    );
    service.add(HOME, CONNECTION, [("Unsaved", Value::from(false))]);
    service.add(
        DEVICE_PATH,
        DEVICE,
        [
            ("DeviceType", Value::from(DEVICE_WIFI)),
            ("Interface", Value::from("wlan0")),
            ("State", Value::from(DEVICE_ACTIVATED)),
        ],
    );
    service.add(
        DEVICE_PATH,
        WIRELESS,
        [(
            "AccessPoints",
            Value::from(vec![ObjectPath::from_static_str_unchecked(POINT_PATH)]),
        )],
    );
    service.add(
        POINT_PATH,
        ACCESS_POINT,
        [
            ("Ssid", Value::from(b"Home".to_vec())),
            ("Strength", Value::from(40_u8)),
            ("Frequency", Value::from(2412_u32)),
        ],
    );
    service
}

fn wait(runtime: &Runtime, network: &Network, ready: impl Fn(&State) -> bool) -> State {
    let mut changes = Box::pin(network.changes());
    let state = network.state();
    if ready(&state) {
        return state;
    }
    runtime.block_on(async {
        tokio::time::timeout(TIMEOUT, async {
            while let Some(state) = changes.next().await {
                if ready(&state) {
                    return state;
                }
            }
            panic!("the network monitor stopped");
        })
        .await
        .expect("the network state should settle")
    })
}

fn profile_ids(state: &State) -> Vec<&str> {
    state
        .snapshot
        .profiles
        .iter()
        .map(|profile| profile.id.as_str())
        .collect()
}

fn strength(state: &State) -> Option<u8> {
    state
        .snapshot
        .access_points
        .first()
        .map(|point| point.strength)
}

#[test]
fn network_follows_signals_and_rereads_only_updated_connections() {
    let Some(bus) = PrivateBus::start() else {
        return;
    };
    bus.stand_in_for_system_bus();
    let service = network_manager();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("tokio runtime should build");
    let network = Network::start();

    let state = wait(&runtime, &network, |state| {
        profile_ids(state) == ["Home"] && strength(state) == Some(40)
    });
    assert_eq!(state.snapshot.hostname, "rikka");
    assert_eq!(
        state.snapshot.access_points[0].saved.as_deref(),
        Some("home")
    );

    service.set(POINT_PATH, ACCESS_POINT, "Strength", Value::from(80_u8));
    wait(&runtime, &network, |state| strength(state) == Some(80));

    service.answer(
        HOME,
        CONNECTION,
        "GetSettings",
        saved("home", "Renamed", "Home"),
    );
    service.set(POINT_PATH, ACCESS_POINT, "Strength", Value::from(60_u8));
    let state = wait(&runtime, &network, |state| strength(state) == Some(60));
    assert_eq!(
        profile_ids(&state),
        ["Home"],
        "a saved connection is read again only when it says it changed"
    );

    service.signal(HOME, CONNECTION, "Updated", &());
    wait(&runtime, &network, |state| {
        profile_ids(state) == ["Renamed"]
    });

    service.answer(
        OFFICE,
        CONNECTION,
        "GetSettings",
        saved("office", "Office", "Office"),
    );
    service.add(OFFICE, CONNECTION, [("Unsaved", Value::from(false))]);
    wait(&runtime, &network, |state| {
        profile_ids(state) == ["Renamed", "Office"]
    });

    service.remove(HOME, CONNECTION);
    service.remove(POINT_PATH, ACCESS_POINT);
    let state = wait(&runtime, &network, |state| {
        profile_ids(state) == ["Office"] && state.snapshot.access_points.is_empty()
    });
    assert!(state.snapshot.wireless.enabled);
}
