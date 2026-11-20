use rikka_ui::{Label, MaterialSymbol};
use rikka_weather::{Condition, State, Temperature};
use shell_feeds::{describe, symbol};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Forecast {
    pub(crate) condition: Condition,
    pub(crate) daytime: bool,
    pub(crate) now: Temperature,
    pub(crate) high: Option<Temperature>,
    pub(crate) low: Option<Temperature>,
    pub(crate) place: String,
}

impl Forecast {
    #[must_use]
    pub(super) fn from_state(state: &State) -> Option<Forecast> {
        let report = state.report.as_ref()?;
        let today = report.today();
        Some(Forecast {
            condition: report.now.condition,
            daytime: report.now.daytime,
            now: report.now.temperature,
            high: today.map(|day| day.high),
            low: today.map(|day| day.low),
            place: state.place.as_ref().unwrap_or(&report.place).name.clone(),
        })
    }

    #[must_use]
    pub(crate) fn symbol(&self) -> MaterialSymbol {
        symbol(self.condition, self.daytime)
    }

    #[must_use]
    pub(crate) fn describe(&self) -> Label {
        describe(self.condition)
    }
}
