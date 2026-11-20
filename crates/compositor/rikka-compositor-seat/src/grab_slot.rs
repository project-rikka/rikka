use std::any::Any;
use std::fmt;
use std::marker::PhantomData;

#[derive(Default)]
pub(crate) struct GrabSlot {
    stored: Option<Box<dyn Any>>,
    grabbed: bool,
    generation: u64,
}

pub(crate) struct TakenGrab<G: ?Sized> {
    stored: Box<dyn Any>,
    generation: u64,
    grab: PhantomData<Box<G>>,
}

impl<G: ?Sized + 'static> TakenGrab<G> {
    pub(crate) fn grab(&mut self) -> &mut G {
        let Some(grab) = self.stored.downcast_mut::<Box<G>>() else {
            unreachable!("a taken grab is only ever the type it was taken as");
        };
        grab
    }

    fn into_grab(self) -> Box<G> {
        let Some(grab) = downcast(self.stored) else {
            unreachable!("a taken grab is only ever the type it was taken as");
        };
        grab
    }
}

pub(crate) enum GrabOutcome<G: ?Sized> {
    Kept,
    Superseded(Box<G>),
    Ended(Box<G>),
}

impl GrabSlot {
    pub(crate) fn grabbed(&self) -> bool {
        self.grabbed
    }

    pub(crate) fn install<G: ?Sized + 'static>(&mut self, grab: Box<G>) -> Option<Box<G>> {
        self.generation += 1;
        self.grabbed = true;
        self.stored.replace(Box::new(grab)).and_then(downcast)
    }

    pub(crate) fn release<G: ?Sized + 'static>(&mut self) -> Option<Option<Box<G>>> {
        if !self.grabbed {
            return None;
        }
        self.generation += 1;
        self.grabbed = false;
        Some(self.stored.take().and_then(downcast))
    }

    pub(crate) fn take<G: ?Sized + 'static>(&mut self) -> Option<TakenGrab<G>> {
        if !self.stored.as_ref()?.is::<Box<G>>() {
            return None;
        }
        Some(TakenGrab {
            stored: self.stored.take()?,
            generation: self.generation,
            grab: PhantomData,
        })
    }

    pub(crate) fn restore<G: ?Sized + 'static>(&mut self, taken: TakenGrab<G>) -> GrabOutcome<G> {
        if self.generation == taken.generation {
            self.stored = Some(taken.stored);
            GrabOutcome::Kept
        } else if self.grabbed {
            GrabOutcome::Superseded(taken.into_grab())
        } else {
            GrabOutcome::Ended(taken.into_grab())
        }
    }
}

impl fmt::Debug for GrabSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrabSlot")
            .field("grabbed", &self.grabbed)
            .field("running", &(self.grabbed && self.stored.is_none()))
            .finish()
    }
}

fn downcast<G: ?Sized + 'static>(stored: Box<dyn Any>) -> Option<Box<G>> {
    stored.downcast::<Box<G>>().ok().map(|boxed| *boxed)
}
