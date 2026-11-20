use rikka_sync::Mutex;
use std::sync::Arc;

use rikka_audio::{AudioEngine, EngineConfig};

static ENGINE: Mutex<Option<Arc<AudioEngine>>> = Mutex::new(None);

pub(crate) fn shared() -> rikka_audio::Result<Arc<AudioEngine>> {
    let mut slot = ENGINE.lock();
    if let Some(engine) = slot.as_ref().filter(|engine| !engine.is_device_lost()) {
        return Ok(Arc::clone(engine));
    }
    let engine = Arc::new(AudioEngine::new(EngineConfig::new())?);
    *slot = Some(Arc::clone(&engine));
    Ok(engine)
}
