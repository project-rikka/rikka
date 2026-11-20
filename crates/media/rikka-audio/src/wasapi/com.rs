use std::marker::PhantomData;

use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

pub(super) struct Apartment {
    owned: bool,
    _thread: PhantomData<*const ()>,
}

impl Apartment {
    pub(super) fn enter() -> windows::core::Result<Apartment> {
        let entered = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if entered == RPC_E_CHANGED_MODE {
            return Ok(Apartment {
                owned: false,
                _thread: PhantomData,
            });
        }
        entered.ok()?;
        Ok(Apartment {
            owned: true,
            _thread: PhantomData,
        })
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.owned {
            unsafe { CoUninitialize() };
        }
    }
}

pub(super) struct Mta<T>(T);

unsafe impl<T> Send for Mta<T> {}

impl<T> Mta<T> {
    pub(super) unsafe fn new(interface: T) -> Mta<T> {
        Mta(interface)
    }

    pub(super) fn get(&self) -> &T {
        &self.0
    }
}
