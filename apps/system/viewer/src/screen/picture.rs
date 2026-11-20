use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use rikka_app::prelude::*;
use viewer_media::Extent;
use viewer_psd::Flat;

use crate::bar;
use crate::screen::canvas::Canvas;
use crate::screen::{Screen, status};

pub(crate) const PROXY_SIDE: u32 = 2560;

#[derive(Clone, Debug)]
pub(crate) enum Proxy {
    Unneeded,
    Making,
    Made { source: ImageSource, width: u32 },
}

#[derive(Debug)]
enum PictureState {
    Drawn {
        source: ImageSource,
        extent: Extent,
        proxy: Proxy,
    },
    Fitted(Rc<Path>),
    Flattening(PathBuf),
    Failed(Label),
}

#[derive(Debug)]
pub(crate) struct Picture {
    state: PictureState,
    zoom: Zoom,
}

impl Picture {
    pub(crate) fn open(path: &Path, extent: Option<Extent>) -> Self {
        Self::showing(match extent {
            Some(extent) => PictureState::Drawn {
                source: ImageSource::file(Arc::<Path>::from(path)),
                proxy: match extent.width.max(extent.height) > PROXY_SIDE {
                    true => Proxy::Making,
                    false => Proxy::Unneeded,
                },
                extent,
            },
            None => PictureState::Fitted(Rc::from(path)),
        })
    }

    pub(crate) fn flatten(path: &Path) -> Self {
        Self::showing(PictureState::Flattening(path.to_path_buf()))
    }

    fn showing(state: PictureState) -> Self {
        Self {
            state,
            zoom: Zoom::Fit,
        }
    }

    pub(crate) fn start(&self) {
        if let PictureState::Drawn {
            source: ImageSource::Path(path),
            proxy: Proxy::Making,
            ..
        } = &self.state
        {
            let path = Arc::clone(path);
            spawn_blocking(
                move || {
                    let proxy = proxy_of(&path);
                    (path, proxy)
                },
                |screen: &mut Screen, (path, proxy): (Arc<Path>, Proxy)| {
                    if let Some(picture) = screen.picture() {
                        picture.proxied(&path, proxy);
                    }
                },
            );
            return;
        }
        let PictureState::Flattening(path) = &self.state else {
            return;
        };
        let path = path.clone();
        spawn_blocking(
            move || {
                let flat = viewer_psd::flatten(&path);
                (path, flat)
            },
            |screen: &mut Screen, (path, flat): (PathBuf, viewer_psd::Result<Flat>)| {
                if let Some(picture) = screen.picture() {
                    picture.flattened(&path, flat);
                }
            },
        );
    }

    fn proxied(&mut self, path: &Path, proxy_state: Proxy) {
        if let PictureState::Drawn {
            source: ImageSource::Path(state),
            proxy,
            ..
        } = &mut self.state
            && state.as_ref() == path
        {
            *proxy = proxy_state;
        }
    }

    fn flattened(&mut self, path: &Path, flat: viewer_psd::Result<Flat>) {
        if !matches!(&self.state, PictureState::Flattening(awaited) if awaited == path) {
            return;
        }
        self.state = match flat {
            Ok(flat) => PictureState::Drawn {
                proxy: Proxy::Unneeded,
                extent: Extent {
                    width: flat.width,
                    height: flat.height,
                },
                source: ImageSource::Rgba {
                    width: flat.width,
                    height: flat.height,
                    pixels: flat.pixels,
                },
            },
            Err(viewer_psd::Error::TooLarge { width, height }) => {
                PictureState::Failed(status::too_large(width, height))
            }
            Err(error) => PictureState::Failed(status::unreadable(&error)),
        };
    }

    pub(crate) fn zoomed(&mut self, zoom: Zoom) {
        self.zoom = zoom;
    }

    pub(crate) fn stage(&self) -> Element {
        match &self.state {
            PictureState::Drawn {
                source,
                extent,
                proxy,
            } => Element::new(Canvas {
                source: source.clone(),
                proxy: proxy.clone(),
                extent: *extent,
                zoom: self.zoom,
            })
            .grow(1.0),
            PictureState::Fitted(path) => {
                container(image(path.as_ref()).fit(ImageFit::ScaleDown).fill())
                    .center()
                    .grow(1.0)
                    .build()
            }
            PictureState::Flattening(_) => container(spinner()).center().grow(1.0).build(),
            PictureState::Failed(reason) => {
                status_page(MaterialSymbol::BROKEN_IMAGE, reason.clone())
                    .grow(1.0)
                    .build()
            }
        }
    }

    pub(crate) fn detail(&self) -> Option<Element> {
        if !matches!(self.state, PictureState::Drawn { .. }) {
            return None;
        }
        let step = |symbol, zoom: Zoom| {
            bar::icon_button(
                symbol,
                move |picture: &mut Picture| picture.zoomed(zoom),
                self.zoom != zoom,
            )
        };

        Some(
            row! {
                bar::detail(self.reading(), palette().text_muted);
                step(MaterialSymbol::FIT_SCREEN, Zoom::Fit);
                step(MaterialSymbol::CENTER_FOCUS_WEAK, Zoom::Scale(1.0));
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .build(),
        )
    }

    fn reading(&self) -> Label {
        match self.zoom {
            Zoom::Fit => tr!("viewer.zoom.fit"),
            Zoom::Scale(scale) => Label::from(format!("{:.0}%", scale * 100.0)),
        }
    }
}

fn proxy_of(path: &Path) -> Proxy {
    let decoded = rikka_image::read(path)
        .decode()
        .and_then(|image| image.thumbnail(PROXY_SIDE, PROXY_SIDE));
    let Ok(image) = decoded else {
        return Proxy::Unneeded;
    };
    let (width, height) = image.dimensions();
    Proxy::Made {
        source: ImageSource::Rgba {
            width,
            height,
            pixels: Arc::from(image.into_rgba8()),
        },
        width,
    }
}
