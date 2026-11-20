use rikka_app::prelude::*;

use rikka_network::{Bars, Security};

pub(crate) fn symbol(bars: Bars, security: Security) -> MaterialSymbol {
    match (bars, security.is_open()) {
        (Bars::Four, true) => MaterialSymbol::WIFI,
        (Bars::Three, true) => MaterialSymbol::NETWORK_WIFI_3_BAR,
        (Bars::Two, true) => MaterialSymbol::WIFI_2_BAR,
        (Bars::One, true) => MaterialSymbol::WIFI_1_BAR,
        (Bars::Four, false) => MaterialSymbol::NETWORK_WIFI_LOCKED,
        (Bars::Three, false) => MaterialSymbol::NETWORK_WIFI_3_BAR_LOCKED,
        (Bars::Two, false) => MaterialSymbol::NETWORK_WIFI_2_BAR_LOCKED,
        (Bars::One, false) => MaterialSymbol::NETWORK_WIFI_1_BAR_LOCKED,
    }
}
