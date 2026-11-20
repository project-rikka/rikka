use rikka_web::MixedMode;

use browser_pages::Certificate;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Security {
    #[default]
    Unknown,
    Insecure,
    Secure(Option<Certificate>),
    Broken {
        certificate: Option<Certificate>,
        mixed: bool,
        exception: bool,
    },
}

impl Security {
    pub fn new(uri: &str, security: &rikka_web::Security) -> Security {
        let certificate = || security.certificate.as_deref().and_then(Certificate::parse);
        if uri.starts_with("http://") {
            return Security::Insecure;
        }
        if !uri.starts_with("https://") {
            return Security::Unknown;
        }
        let mixed = security.mixed_active == MixedMode::ContentLoaded
            || security.mixed_display == MixedMode::ContentLoaded;
        match (security.secure, mixed, security.exception) {
            (true, false, false) => Security::Secure(certificate()),
            (secure, mixed, exception) if secure || exception || security.certificate.is_some() => {
                Security::Broken {
                    certificate: certificate(),
                    mixed,
                    exception,
                }
            }
            _ => Security::Unknown,
        }
    }
}
