use libloading::Library;

#[cfg(unix)]
pub(crate) const AVUTIL: &str = "libavutil.so.61";
#[cfg(unix)]
pub(crate) const AVCODEC: &str = "libavcodec.so.63";
#[cfg(unix)]
pub(crate) const AVFORMAT: &str = "libavformat.so.63";
#[cfg(unix)]
pub(crate) const SWSCALE: &str = "libswscale.so.10";

#[cfg(windows)]
pub(crate) const AVUTIL: &str = "avutil-61.dll";
#[cfg(windows)]
pub(crate) const AVCODEC: &str = "avcodec-63.dll";
#[cfg(windows)]
pub(crate) const AVFORMAT: &str = "avformat-63.dll";
#[cfg(windows)]
pub(crate) const SWSCALE: &str = "swscale-10.dll";

#[cfg(unix)]
unsafe fn load(soname: &str) -> Result<Library, libloading::Error> {
    unsafe { Library::new(soname) }
}

#[cfg(windows)]
unsafe fn load(soname: &str) -> Result<Library, libloading::Error> {
    use libloading::os::windows::{
        LOAD_LIBRARY_SEARCH_APPLICATION_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
    };
    let flags = LOAD_LIBRARY_SEARCH_APPLICATION_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32;
    unsafe { libloading::os::windows::Library::load_with_flags(soname, flags) }.map(Library::from)
}

pub(crate) fn open(soname: &str) -> Result<&'static Library, String> {
    let library = unsafe { load(soname) }
        .map_err(|error| format!("{soname} could not be opened: {error}"))?;
    Ok(Box::leak(Box::new(library)))
}

pub(crate) unsafe fn symbol<T: Copy>(
    library: &Library,
    soname: &str,
    name: &str,
) -> Result<T, String> {
    unsafe { library.get::<T>(name.as_bytes()) }
        .map(|symbol| *symbol)
        .map_err(|error| format!("{soname} has no `{name}`: {error}"))
}

macro_rules! symbols {
    (
        $soname:expr;
        $(
            fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)?;
        )*
    ) => {
        struct Symbols {
            $($name: unsafe extern "C" fn($($ty),*) $(-> $ret)?,)*
        }

        static SYMBOLS: ::std::sync::OnceLock<Result<Symbols, String>> =
            ::std::sync::OnceLock::new();

        pub fn load() -> Result<(), String> {
            resolve().map(|_| ())
        }

        fn resolve() -> Result<&'static Symbols, String> {
            SYMBOLS
                .get_or_init(|| {
                    let library = $crate::symbols::open($soname)?;
                    Ok(Symbols {
                        $($name: unsafe {
                            $crate::symbols::symbol(
                                library,
                                $soname,
                                stringify!($name),
                            )?
                        },)*
                    })
                })
                .as_ref()
                .map_err(Clone::clone)
        }

        fn resolved() -> &'static Symbols {
            match SYMBOLS.get() {
                Some(Ok(symbols)) => symbols,
                _ => unopened(),
            }
        }

        #[cold]
        #[inline(never)]
        fn unopened() -> ! {
            panic!("`rikka_ffmpeg::load` opens FFmpeg before anything calls into it")
        }

        $(
            pub unsafe fn $name($($arg: $ty),*) $(-> $ret)? {
                unsafe { (resolved().$name)($($arg),*) }
            }
        )*
    };
}

pub(crate) use symbols;
