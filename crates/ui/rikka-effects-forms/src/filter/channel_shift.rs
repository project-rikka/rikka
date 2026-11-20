use crate::form::Form;
use crate::rows;
use crate::tuner::Tuner;
use rikka_effects::filter::channel_shift::{ANGLE, ChannelShift, DISTANCE};
use rikka_ui::Element;

impl Form for ChannelShift {
    fn form<S: 'static>(tuner: &Tuner<S, Self>) -> Element {
        rows::form_sheet(vec![tuner.angle(ANGLE, false), tuner.amount(DISTANCE)])
    }
}
