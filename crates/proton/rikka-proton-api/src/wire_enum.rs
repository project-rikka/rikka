#[macro_export]
macro_rules! wire_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident $(: default $default:ident)? {
            $($variant:ident = $value:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(from = "i32", into = "i32")]
        $vis enum $name {
            $($variant,)+
            Other(i32),
        }

        $(
            impl Default for $name {
                fn default() -> $name {
                    $name::$default
                }
            }
        )?

        impl From<i32> for $name {
            fn from(value: i32) -> $name {
                match value {
                    $($value => $name::$variant,)+
                    other => $name::Other(other),
                }
            }
        }

        impl From<$name> for i32 {
            fn from(value: $name) -> i32 {
                match value {
                    $($name::$variant => $value,)+
                    $name::Other(other) => other,
                }
            }
        }
    };
}
