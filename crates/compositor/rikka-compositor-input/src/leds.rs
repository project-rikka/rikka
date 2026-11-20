use input::Led;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
    pub struct Leds: u8 {
        const NUM_LOCK = 1;
        const CAPS_LOCK = 1 << 1;
        const SCROLL_LOCK = 1 << 2;
    }
}

impl Leds {
    pub(crate) fn libinput(self) -> Led {
        let mut led = Led::empty();
        led.set(Led::NUMLOCK, self.contains(Self::NUM_LOCK));
        led.set(Led::CAPSLOCK, self.contains(Self::CAPS_LOCK));
        led.set(Led::SCROLLLOCK, self.contains(Self::SCROLL_LOCK));
        led
    }
}
