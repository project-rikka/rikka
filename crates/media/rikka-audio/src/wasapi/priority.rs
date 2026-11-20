use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW,
};
use windows::core::w;

pub(super) struct ProAudio(HANDLE);

impl ProAudio {
    pub(super) fn join() -> Option<ProAudio> {
        let mut index = 0;
        match unsafe { AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut index) } {
            Ok(task) => Some(ProAudio(task)),
            Err(error) => {
                tracing::warn!(%error, "audio thread runs without MMCSS scheduling");
                None
            }
        }
    }
}

impl Drop for ProAudio {
    fn drop(&mut self) {
        if let Err(error) = unsafe { AvRevertMmThreadCharacteristics(self.0) } {
            tracing::debug!(%error, "failed to leave MMCSS scheduling");
        }
    }
}
