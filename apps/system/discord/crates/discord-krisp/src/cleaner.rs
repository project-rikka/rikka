use std::path::Path;
use std::sync::Arc;

use crate::FRAME;
use crate::abi::{self, NcConfig, Session, WidePath};
use crate::error::{Error, Result};
use crate::sdk::Loaded;

const FULL_SUPPRESSION: f32 = 100.0;

pub struct NoiseCleaner {
    loaded: Arc<Loaded>,
    session: Session,
    output: [f32; FRAME],
}

unsafe impl Send for NoiseCleaner {}

impl NoiseCleaner {
    pub(crate) fn open(loaded: Arc<Loaded>, model: &Path) -> Result<NoiseCleaner> {
        let path = WidePath::new(model)?;
        let info = path.model();
        let config = NcConfig {
            input_sample_rate: crate::SAMPLE_RATE as i32,
            input_frame_duration: abi::FRAME_MS,
            output_sample_rate: crate::SAMPLE_RATE as i32,
            model: &info,
            session_stats: false,
            ringtone: std::ptr::null(),
        };
        let session = unsafe { (loaded.create_nc)(&config) };
        if session.is_null() {
            return Err(Error::Session("noise cancellation"));
        }
        Ok(NoiseCleaner {
            loaded,
            session,
            output: [0.0; FRAME],
        })
    }

    pub fn process(&mut self, frame: &mut [f32; FRAME]) -> Result<()> {
        let status = unsafe {
            (self.loaded.process_nc)(
                self.session,
                frame.as_ptr(),
                FRAME,
                self.output.as_mut_ptr(),
                FRAME,
                FULL_SUPPRESSION,
                std::ptr::null_mut(),
            )
        };
        if status != 0 {
            return Err(Error::Process(status));
        }
        frame.copy_from_slice(&self.output);
        Ok(())
    }
}

impl Drop for NoiseCleaner {
    fn drop(&mut self) {
        unsafe { (self.loaded.destroy_nc)(self.session) };
    }
}

impl std::fmt::Debug for NoiseCleaner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NoiseCleaner")
            .finish_non_exhaustive()
    }
}
