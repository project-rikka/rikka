use crate::seat_event::SeatEvent;
use crate::seats::Seats;

pub trait SeatHost: Sized + 'static {
    fn seats(&mut self) -> &mut Seats;

    fn handle_event(&mut self, event: SeatEvent);
}
