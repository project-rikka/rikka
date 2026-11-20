use rikka_effects::Scale;
use rikka_widgets::{NumberField, Slider, number_field, slider};

pub trait ScaleControls {
    fn field(&self, value: f32) -> NumberField;

    fn slider(&self, value: f32) -> Slider;
}

impl ScaleControls for Scale {
    fn field(&self, value: f32) -> NumberField {
        self.unit
            .symbol()
            .into_iter()
            .fold(number_field(value), NumberField::unit)
            .range(self.range.clone())
            .decimals(self.decimals)
            .step(10f32.powi(-i32::from(self.decimals.min(2))))
    }

    fn slider(&self, value: f32) -> Slider {
        let track = self.track();
        slider(value.clamp(*track.start(), *track.end()))
            .range(track)
            .step(self.step())
    }
}
