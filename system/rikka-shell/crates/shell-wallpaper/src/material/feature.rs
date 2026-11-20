use std::path::PathBuf;
use std::sync::Arc;

use rikka_seam::SeamClient;
use rikka_settings::session::appearance::{MATERIAL_SOURCE, UNSET};
use rikka_sync::Mutex;
use shell_kit::{Feature, Shell, appearance};

#[derive(Debug, Default)]
pub struct MaterialTheme;

impl MaterialTheme {
    #[must_use]
    pub fn new() -> MaterialTheme {
        MaterialTheme
    }
}

impl Feature for MaterialTheme {
    fn install(self, shell: Shell) -> Shell {
        let seed_request = SeedRequest::new(shell.seam());
        shell.follow(
            appearance::DOMAIN,
            appearance::WALLPAPER,
            move |_, value: String| seed_request.read(crate::file_of_setting(value)),
        )
    }
}

#[derive(Debug)]
struct SeedRequest {
    seam: SeamClient,
    pending: Arc<Mutex<PendingRead>>,
}

#[derive(Debug, Default)]
struct PendingRead {
    next: Option<Option<PathBuf>>,
    reading: bool,
}

impl SeedRequest {
    fn new(seam: SeamClient) -> SeedRequest {
        SeedRequest {
            seam,
            pending: Arc::new(Mutex::new(PendingRead::default())),
        }
    }

    fn read(&self, wallpaper: Option<PathBuf>) {
        let mut state = self.pending.lock();
        state.next = Some(wallpaper);
        if state.reading {
            return;
        }
        state.reading = true;
        drop(state);

        let seam = self.seam.clone();
        let pending = Arc::clone(&self.pending);
        let spawned = std::thread::Builder::new()
            .name("rikka-shell-material".to_owned())
            .spawn(move || {
                while let Some(wallpaper) = take(&pending) {
                    write(&seam, wallpaper);
                }
            });
        if let Err(error) = spawned {
            self.pending.lock().reading = false;
            tracing::warn!("cannot extract wallpaper colour: {error}");
        }
    }
}

fn take(pending: &Mutex<PendingRead>) -> Option<Option<PathBuf>> {
    let mut state = pending.lock();
    let next = state.next.take();
    state.reading = next.is_some();
    next
}

fn write(seam: &SeamClient, wallpaper: Option<PathBuf>) {
    let Some(path) = wallpaper else {
        return seam.notify(appearance::write(MATERIAL_SOURCE, UNSET));
    };
    match rikka_theme_seed::seed(&path) {
        Ok(source) => {
            tracing::info!(
                "theme source colour of {} is {}",
                path.display(),
                source.to_hex()
            );
            seam.notify(appearance::write(MATERIAL_SOURCE, source.to_hex()));
        }
        Err(error) => tracing::warn!("failed to derive theme from {}: {error}", path.display()),
    }
}
