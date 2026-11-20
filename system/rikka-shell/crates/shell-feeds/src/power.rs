use rikka_power::Action;
use rikka_ui::MaterialSymbol;

#[must_use]
pub fn power_symbol(action: Action) -> MaterialSymbol {
    match action {
        Action::PowerOff => MaterialSymbol::POWER_SETTINGS_NEW,
        Action::Reboot => MaterialSymbol::RESTART_ALT,
        Action::Suspend => MaterialSymbol::BEDTIME,
    }
}
