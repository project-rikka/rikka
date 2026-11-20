use rikka_power::{Action, PowerCapabilities};
use rikka_ui::{Element, View};
use shell_feeds::power_symbol;

use crate::centre::state::Centre;
use crate::centre::view::tile::round;

pub(crate) fn actions(capabilities: PowerCapabilities) -> Vec<Element> {
    Action::ALL
        .into_iter()
        .filter(|action| capabilities.allows(*action))
        .map(|action| {
            round(power_symbol(action), move |centre: &mut Centre| {
                centre.ask(action)
            })
            .build()
        })
        .collect()
}
