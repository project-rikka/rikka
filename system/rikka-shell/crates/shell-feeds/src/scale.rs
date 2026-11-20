use futures_util::Stream;
use rikka_weather::{TemperatureUnit, Units};
use rikka_worker::Observable;

#[derive(Clone, Debug)]
pub struct Scale(Observable<TemperatureUnit>);

impl Default for Scale {
    fn default() -> Scale {
        Scale(Observable::new(Units::for_locale().temperature))
    }
}

impl Scale {
    #[must_use]
    pub fn get(&self) -> TemperatureUnit {
        self.0.get()
    }

    pub(crate) fn chosen(&self, name: &str) {
        self.0
            .update(TemperatureUnit::named(name).unwrap_or(Units::for_locale().temperature));
    }

    pub fn changes(&self) -> impl Stream<Item = TemperatureUnit> + Send + 'static {
        self.0.changes()
    }
}
