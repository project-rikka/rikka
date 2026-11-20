use rikka_sync::Mutex;
use std::ffi::{CStr, c_char, c_int};
use std::path::Path;
use std::sync::Arc;

use libloading::os::unix::{Library, RTLD_LAZY, RTLD_LOCAL, Symbol};

use crate::abi::{self, CreateNc, CreateVad, Destroy, GlobalInit, ProcessNc, ProcessVad, WidePath};
use crate::cleaner::NoiseCleaner;
use crate::detector::VoiceDetector;
use crate::error::{Error, Result};
use crate::module::Module;
use crate::runtime;

static LOADED: Mutex<Option<Arc<Loaded>>> = Mutex::new(None);

#[derive(Clone)]
pub struct Sdk {
    loaded: Arc<Loaded>,
}

pub(crate) struct Loaded {
    pub(crate) create_nc: CreateNc,
    pub(crate) process_nc: ProcessNc,
    pub(crate) destroy_nc: Destroy,
    pub(crate) create_vad: CreateVad,
    pub(crate) process_vad: ProcessVad,
    pub(crate) destroy_vad: Destroy,
    module: Module,
    _library: Library,
    _runtime: Library,
}

impl Sdk {
    pub fn load(module: &Module, working: &Path) -> Result<Sdk> {
        let mut loaded = LOADED.lock();
        let shared = match loaded.as_ref() {
            Some(shared) if shared.module.folder() == module.folder() => Arc::clone(shared),
            Some(shared) => {
                return Err(Error::AlreadyLoaded(shared.module.folder().to_path_buf()));
            }
            None => {
                let shared = Arc::new(Loaded::open(module, working)?);
                *loaded = Some(Arc::clone(&shared));
                shared
            }
        };
        Ok(Sdk { loaded: shared })
    }

    pub fn noise_cleaner(&self) -> Result<NoiseCleaner> {
        NoiseCleaner::open(Arc::clone(&self.loaded), self.loaded.module.noise_model())
    }

    pub fn voice_detector(&self) -> Result<VoiceDetector> {
        VoiceDetector::open(Arc::clone(&self.loaded), self.loaded.module.voice_model())
    }
}

impl std::fmt::Debug for Sdk {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Sdk")
            .field("module", &self.loaded.module)
            .finish_non_exhaustive()
    }
}

impl Loaded {
    fn open(module: &Module, working: &Path) -> Result<Loaded> {
        std::fs::create_dir_all(working)?;
        let runtime = runtime::preload()?;
        let library = unsafe {
            Library::open(
                Some(module.library()),
                RTLD_LAZY | RTLD_LOCAL | libc::RTLD_DEEPBIND,
            )
        }
        .map_err(|error| Error::Load(error.to_string()))?;
        let init: GlobalInit = unsafe { symbol(&library, "krispGlobalInit")? };
        let loaded = Loaded {
            create_nc: unsafe { symbol(&library, "krispCreateNcFloat")? },
            process_nc: unsafe { symbol(&library, "krispProcessNcFloat")? },
            destroy_nc: unsafe { symbol(&library, "krispDestroyNc")? },
            create_vad: unsafe { symbol(&library, "krispCreateVadFloat")? },
            process_vad: unsafe { symbol(&library, "krispProcessVadFloat")? },
            destroy_vad: unsafe { symbol(&library, "krispDestroyVad")? },
            module: module.clone(),
            _library: library,
            _runtime: runtime,
        };
        let working = WidePath::new(working)?;
        let status = unsafe { init(working.as_ptr(), Some(log), abi::LOG_OFF) };
        if status != 0 {
            return Err(Error::Init(status));
        }
        Ok(loaded)
    }
}

unsafe fn symbol<T: Copy>(library: &Library, name: &'static str) -> Result<T> {
    let resolved: Symbol<T> =
        unsafe { library.get(name.as_bytes()) }.map_err(|_| Error::Symbol(name))?;
    Ok(*resolved)
}

unsafe extern "C" fn log(message: *const c_char, level: c_int) {
    if message.is_null() {
        return;
    }
    let message = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    tracing::debug!(level, "krisp: {message}");
}
