#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
enum Bytes {
    Text(String),
    Raw(Vec<u8>),
}

impl Bytes {
    fn from_path(path: &std::path::Path) -> Self {
        use std::os::unix::ffi::OsStrExt;

        let bytes = path.as_os_str().as_bytes();
        match std::str::from_utf8(bytes) {
            Ok(text) => Self::Text(text.to_owned()),
            Err(_) => Self::Raw(bytes.to_vec()),
        }
    }

    fn into_path(self) -> std::path::PathBuf {
        use std::os::unix::ffi::OsStringExt;

        let bytes = match self {
            Self::Text(text) => text.into_bytes(),
            Self::Raw(bytes) => bytes,
        };
        std::path::PathBuf::from(std::ffi::OsString::from_vec(bytes))
    }
}

pub(crate) mod maybe_path {
    use std::path::PathBuf;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::Bytes;

    pub(crate) fn serialize<S: Serializer>(
        path: &Option<PathBuf>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        path.as_deref().map(Bytes::from_path).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<PathBuf>, D::Error> {
        Ok(Option::<Bytes>::deserialize(deserializer)?.map(Bytes::into_path))
    }
}

pub(crate) mod maybe_identifier {
    use rikka_types::WindowIdentifier;
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(crate) fn serialize<S: Serializer>(
        identifier: &Option<WindowIdentifier>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        identifier
            .as_ref()
            .map(ToString::to_string)
            .serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<WindowIdentifier>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(|text| text.parse().map_err(D::Error::custom))
            .transpose()
    }
}

pub(crate) mod paths {
    use std::path::PathBuf;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::Bytes;

    pub(crate) fn serialize<S: Serializer>(
        paths: &[PathBuf],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        paths
            .iter()
            .map(|path| Bytes::from_path(path))
            .collect::<Vec<Bytes>>()
            .serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<PathBuf>, D::Error> {
        Ok(Vec::<Bytes>::deserialize(deserializer)?
            .into_iter()
            .map(Bytes::into_path)
            .collect())
    }
}
