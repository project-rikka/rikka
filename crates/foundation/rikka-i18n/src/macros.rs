#[macro_export]
macro_rules! embedded {
    () => {
        $crate::I18n::builder().embedded($crate::catalogs!())
    };
}

#[macro_export]
macro_rules! args {
    () => { $crate::Args::new() };
    ($($name:ident = $value:expr),+ $(,)?) => {
        $crate::Args::from([$((::core::stringify!($name), $crate::Value::from($value))),+])
    };
}

#[macro_export]
macro_rules! tr {
    ($key:expr $(,)?) => { $crate::Label::translated($key) };
    ($key:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::Label::formatted($key, &$crate::args![$($name = $value),+])
    };
}
