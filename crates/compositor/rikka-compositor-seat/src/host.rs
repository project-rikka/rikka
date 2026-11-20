use rikka_compositor_server::DisplayHandle;
use rikka_compositor_surface::SurfaceHost;

use crate::event::SeatEvent;
use crate::keyboard::{KeyFilter, Keystroke};
use crate::seats::Seats;

pub trait SeatHost: SurfaceHost {
    fn seats(&mut self) -> &mut Seats;

    fn display(&self) -> &DisplayHandle<Self>;

    fn filter_key(&mut self, _keystroke: &Keystroke) -> KeyFilter {
        KeyFilter::Forward
    }

    fn seat_event(&mut self, _event: SeatEvent) {}
}
