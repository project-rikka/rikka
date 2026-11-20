use http::HeaderMap;

#[derive(Debug, Clone)]
pub struct Response<T> {
    value: T,
    headers: HeaderMap,
}

impl<T> Response<T> {
    pub(crate) fn new(value: T, headers: HeaderMap) -> Response<T> {
        Response { value, headers }
    }

    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }

    #[must_use]
    pub fn header(&self, name: impl AsRef<str>) -> Option<&str> {
        self.headers
            .get(name.as_ref())
            .and_then(|value| value.to_str().ok())
    }
}
