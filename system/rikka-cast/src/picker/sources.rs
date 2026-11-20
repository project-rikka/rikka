use std::sync::Arc;

use rikka_capture::{Screen, Shot, ShotOptions, Source};
use rikka_image::Image;
use rikka_seam::net::SocketClient;
use rikka_session::toplevels::{State, ToplevelsClient};

use crate::protocol::{CastSource, SourceKind};

const PREVIEW_WIDTH: u32 = 640;
const PREVIEW_HEIGHT: u32 = 400;

#[derive(Clone, Debug)]
pub(crate) struct Preview {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Arc<[u8]>,
}

#[derive(Clone, Debug)]
pub(crate) struct SourceEntry {
    pub(crate) source: CastSource,
    pub(crate) title: String,
    pub(crate) detail: String,
    pub(crate) preview: Option<Preview>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Listing {
    pub(crate) monitors: Vec<SourceEntry>,
    pub(crate) windows: Vec<SourceEntry>,
    pub(crate) failure: Option<String>,
}

pub(crate) fn list(kinds: &[SourceKind], own_app_id: &str) -> Listing {
    let screen = match Screen::connect() {
        Ok(screen) => screen,
        Err(error) => {
            return Listing {
                failure: Some(error.to_string()),
                ..Listing::default()
            };
        }
    };

    let monitors = match kinds.contains(&SourceKind::Monitor) {
        false => Vec::new(),
        true => screen
            .outputs()
            .into_iter()
            .map(|output| SourceEntry {
                source: CastSource::monitor(&output.name),
                title: match output.description.is_empty() {
                    true => output.name.clone(),
                    false => output.description.clone(),
                },
                detail: format!("{} × {}", output.width, output.height),
                preview: None,
            })
            .collect(),
    };

    let windows = match kinds.contains(&SourceKind::Window) {
        false => Vec::new(),
        true => {
            let stacking = toplevel_state();
            let mut entry: Vec<(usize, SourceEntry)> = screen
                .windows()
                .into_iter()
                .filter(|window| window.app_id != own_app_id)
                .filter_map(|window| {
                    let index = stacking_index(stacking.as_ref(), &window.identifier)?;
                    Some((
                        index,
                        SourceEntry {
                            source: CastSource::window(&window.identifier),
                            title: window.title.clone(),
                            detail: window.app_id.clone(),
                            preview: None,
                        },
                    ))
                })
                .collect();
            entry.sort_by_key(|(index, _)| std::cmp::Reverse(*index));
            entry.into_iter().map(|(_, entry)| entry).collect()
        }
    };

    Listing {
        monitors,
        windows,
        failure: None,
    }
}

fn toplevel_state() -> Option<State> {
    let state = SocketClient::from_env()
        .map_err(|error| error.to_string())
        .and_then(|socket| {
            ToplevelsClient::new(socket.client())
                .state()
                .blocking_wait()
                .and_then(|payload| payload.decode::<State>())
                .map_err(|error| error.to_string())
        });
    match state {
        Ok(state) => Some(state),
        Err(error) => {
            eprintln!("rikka-cast-picker: cannot query window stacking order: {error}");
            None
        }
    }
}

fn stacking_index(stacking: Option<&State>, identifier: &str) -> Option<usize> {
    let Some(state) = stacking else {
        return Some(0);
    };
    state
        .toplevels
        .iter()
        .enumerate()
        .find(|(_, toplevel)| toplevel.id == identifier)
        .filter(|(_, toplevel)| toplevel.on_screen && toplevel.bounds.area() > 0)
        .map(|(at, _)| at)
}

pub(crate) fn preview(source: &CastSource) -> Option<Preview> {
    let capture_source = match source.kind {
        SourceKind::Monitor => Source::Output(source.id.clone()),
        SourceKind::Window => Source::Window(source.id.clone()),
    };
    let shot = match Shot::take(&capture_source, ShotOptions::default()) {
        Ok(shot) => shot,
        Err(error) => {
            eprintln!(
                "rikka-cast-picker: cannot capture preview of {}: {error}",
                source.id
            );
            return None;
        }
    };
    let full = Image::from_rgba8(shot.width, shot.height, shot.pixels).ok()?;
    let small = full.thumbnail(PREVIEW_WIDTH, PREVIEW_HEIGHT).ok()?;
    let (width, height) = small.dimensions();
    Some(Preview {
        width,
        height,
        pixels: Arc::from(small.into_rgba8()),
    })
}
