use futures_channel::oneshot;
use music_core::{Error, Result};
use music_library::Library;

const THREAD: &str = "rikka-music-local";

pub(crate) async fn reading<T: Send + 'static>(
    library: &Library,
    work: impl FnOnce(&Library) -> Result<T> + Send + 'static,
) -> Result<T> {
    let library = library.clone();
    let (reply, answer) = oneshot::channel();
    std::thread::Builder::new()
        .name(THREAD.into())
        .spawn(move || {
            let _ = reply.send(work(&library));
        })
        .map_err(|error| Error::Unreachable(error.to_string()))?;
    answer
        .await
        .map_err(|_| Error::Unreachable(music_library::Error::Stopped.to_string()))?
}

pub(crate) fn failed(error: music_library::Error) -> Error {
    match error {
        music_library::Error::UnknownTrack(id) => Error::Missing(format!("track {id}")),
        other => Error::Unreachable(other.to_string()),
    }
}
