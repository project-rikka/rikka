use rikka_bluetooth::State as BluetoothState;
use rikka_network::State as NetworkState;
use rikka_ui::{MaterialSymbol, tr};

use crate::centre::state::Centre;
use crate::centre::view::network;
use crate::centre::view::quick::{Quick, quick};

pub(crate) fn tile(network: &NetworkState, bluetooth: &BluetoothState) -> Quick {
    let silent = silent([
        network::pressable(network).then(|| network.is_wireless_on()),
        bluetooth.radio().map(|_| bluetooth.is_on()),
    ]);
    let on = silent.unwrap_or_default();

    quick(symbol(on), tr!("shell.centre.airplane"))
        .on(on)
        .here(silent.is_some())
        .toggle(move |centre: &mut Centre| centre.airplane(!on))
}

fn silent(radios: [Option<bool>; 2]) -> Option<bool> {
    radios
        .iter()
        .any(Option::is_some)
        .then(|| radios.iter().flatten().all(|transmitting| !transmitting))
}

fn symbol(on: bool) -> MaterialSymbol {
    match on {
        true => MaterialSymbol::AIRPLANEMODE_ACTIVE,
        false => MaterialSymbol::AIRPLANEMODE_INACTIVE,
    }
}
