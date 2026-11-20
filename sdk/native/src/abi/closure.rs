use std::ffi::c_void;
use std::marker::PhantomData;

use crate::abi::violation;
use crate::ffi::DestroyFn;

#[derive(Debug)]
pub(crate) struct ThisThread(PhantomData<*const ()>);

#[derive(Debug)]
pub(crate) struct AnyThread;

pub(crate) struct Closure<F, S> {
    function: F,
    userdata: *mut c_void,
    destroy: Option<DestroyFn>,
    threads: PhantomData<S>,
}

impl<F: Copy, S> Closure<F, S> {
    pub(crate) unsafe fn new(
        function: F,
        userdata: *mut c_void,
        destroy: Option<DestroyFn>,
    ) -> Self {
        Self {
            function,
            userdata,
            destroy,
            threads: PhantomData,
        }
    }

    pub(crate) fn function(&self) -> F {
        self.function
    }

    pub(crate) fn userdata(&self) -> *mut c_void {
        self.userdata
    }
}

impl<F, S> Drop for Closure<F, S> {
    fn drop(&mut self) {
        if let Some(destroy) = self.destroy {
            unsafe { destroy(self.userdata) }
        }
    }
}

unsafe impl<F: Send> Send for Closure<F, AnyThread> {}

unsafe impl<F: Sync> Sync for Closure<F, AnyThread> {}

impl<F, S> std::fmt::Debug for Closure<F, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Closure")
            .field("userdata", &self.userdata)
            .finish_non_exhaustive()
    }
}

pub(crate) fn required<F>(function: Option<F>, param: &str) -> F {
    function.unwrap_or_else(|| violation(format!("`{param}` is NULL")))
}
