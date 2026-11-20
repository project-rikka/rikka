use url::Url;

use crate::error::{Error, Result};

pub trait IntoUrl: sealed::Sealed {
    fn into_url(self) -> Result<Url>;
}

impl IntoUrl for Url {
    fn into_url(self) -> Result<Url> {
        match self.scheme() {
            "http" | "https" | "ws" | "wss" => Ok(self),
            scheme => Err(Error::InvalidUrl(format!(
                "\"{scheme}\" is not a scheme this client speaks"
            ))),
        }
    }
}

impl IntoUrl for &Url {
    fn into_url(self) -> Result<Url> {
        self.clone().into_url()
    }
}

impl IntoUrl for &str {
    fn into_url(self) -> Result<Url> {
        Url::parse(self)
            .map_err(|error| Error::InvalidUrl(format!("\"{self}\": {error}")))?
            .into_url()
    }
}

impl IntoUrl for &String {
    fn into_url(self) -> Result<Url> {
        self.as_str().into_url()
    }
}

impl IntoUrl for String {
    fn into_url(self) -> Result<Url> {
        self.as_str().into_url()
    }
}

mod sealed {
    pub trait Sealed {}

    impl Sealed for url::Url {}
    impl Sealed for &url::Url {}
    impl Sealed for &str {}
    impl Sealed for &String {}
    impl Sealed for String {}
}
