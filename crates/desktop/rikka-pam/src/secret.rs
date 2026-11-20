use zeroize::Zeroizing;

pub struct Secret(Zeroizing<String>);

impl Secret {
    #[must_use]
    pub fn new(secret: String) -> Secret {
        Secret(Zeroizing::new(secret))
    }

    pub(crate) fn reveal(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Secret").finish_non_exhaustive()
    }
}
