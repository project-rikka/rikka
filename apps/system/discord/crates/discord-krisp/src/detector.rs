use std::path::Path;
use std::sync::Arc;

use crate::FRAME;
use crate::abi::{self, Session, VadConfig, WidePath};
use crate::error::{Error, Result};
use crate::sdk::Loaded;

pub struct VoiceDetector {
    loaded: Arc<Loaded>,
    session: Session,
}

unsafe impl Send for VoiceDetector {}

impl VoiceDetector {
    pub(crate) fn open(loaded: Arc<Loaded>, model: &Path) -> Result<VoiceDetector> {
        let path = WidePath::new(model)?;
        let info = path.model();
        let config = VadConfig {
            input_sample_rate: crate::SAMPLE_RATE as i32,
            input_frame_duration: abi::FRAME_MS,
            model: &info,
        };
        let session = unsafe { (loaded.create_vad)(&config) };
        if session.is_null() {
            return Err(Error::Session("voice activity"));
        }
        Ok(VoiceDetector { loaded, session })
    }

    pub fn probability(&mut self, frame: &[f32; FRAME]) -> Result<f32> {
        let mut probability = 0.0;
        let status = unsafe {
            (self.loaded.process_vad)(self.session, frame.as_ptr(), FRAME, &mut probability)
        };
        if status != 0 {
            return Err(Error::Process(status));
        }
        Ok(probability.clamp(0.0, 1.0))
    }
}

impl Drop for VoiceDetector {
    fn drop(&mut self) {
        unsafe { (self.loaded.destroy_vad)(self.session) };
    }
}

impl std::fmt::Debug for VoiceDetector {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VoiceDetector")
            .finish_non_exhaustive()
    }
}
