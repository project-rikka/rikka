use music_core::{AudioCache, Providers, Result, Roots};
use music_library::Library;
use music_local::Local;
use rikka_hifi::Hifi;

pub(crate) fn services(
    roots: Roots,
    audio: &AudioCache,
    output: &Hifi,
    library: Option<Library>,
) -> Result<Providers> {
    let providers = Providers::new(roots, audio, output);
    let providers = match library {
        Some(library) => {
            providers.try_provider(Local::SERVICE, |setup| Local::new(setup, library))?
        }
        None => providers,
    };
    Ok(providers
        .adapter::<music_spotify::Spotify>()
        .adapter::<music_youtube::YouTubeMusic>()
        .adapter::<music_qobuz::Qobuz>()
        .adapter::<music_soundcloud::SoundCloud>())
}
