use std::marker::PhantomData;

const FLUSH_TO_ZERO: u32 = 1 << 15;
const DENORMALS_ARE_ZERO: u32 = 1 << 6;

#[must_use = "denormals are flushed only while the guard lives"]
pub struct FlushDenormals {
    saved: u32,
    thread_bound: PhantomData<*const ()>,
}

impl FlushDenormals {
    pub fn new() -> FlushDenormals {
        let saved = read();
        write(saved | FLUSH_TO_ZERO | DENORMALS_ARE_ZERO);
        FlushDenormals {
            saved,
            thread_bound: PhantomData,
        }
    }
}

impl Default for FlushDenormals {
    fn default() -> FlushDenormals {
        FlushDenormals::new()
    }
}

impl Drop for FlushDenormals {
    fn drop(&mut self) {
        write(self.saved);
    }
}

impl std::fmt::Debug for FlushDenormals {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FlushDenormals")
            .finish_non_exhaustive()
    }
}

fn read() -> u32 {
    let mut value: u32 = 0;
    unsafe {
        core::arch::asm!(
            "stmxcsr [{}]",
            in(reg) &raw mut value,
            options(nostack, preserves_flags)
        );
    }
    value
}

fn write(value: u32) {
    unsafe {
        core::arch::asm!(
            "ldmxcsr [{}]",
            in(reg) &raw const value,
            options(nostack, readonly, preserves_flags)
        );
    }
}
