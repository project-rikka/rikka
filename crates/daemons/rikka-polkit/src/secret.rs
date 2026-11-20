pub struct Secret(String);

impl Secret {
    #[must_use]
    pub fn new(secret: String) -> Secret {
        Secret(secret)
    }

    pub(crate) fn reveal(&self) -> &str {
        &self.0
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        unsafe {
            let bytes = self.0.as_bytes_mut();
            for byte in bytes {
                std::ptr::write_volatile(byte, 0);
            }
        }
        std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secret").finish_non_exhaustive()
    }
}
