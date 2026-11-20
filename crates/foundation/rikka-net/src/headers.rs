use http::{HeaderName, HeaderValue};

use crate::error::{Error, Result};

pub(crate) fn pair<K, V>(name: K, value: V) -> Result<(HeaderName, HeaderValue)>
where
    K: TryInto<HeaderName>,
    V: TryInto<HeaderValue>,
{
    let name = name
        .try_into()
        .map_err(|_| Error::InvalidHeader("invalid header name".to_owned()))?;
    let value = value
        .try_into()
        .map_err(|_| Error::InvalidHeader("invalid header value".to_owned()))?;

    Ok((name, value))
}
