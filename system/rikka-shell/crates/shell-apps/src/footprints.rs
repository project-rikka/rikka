use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};

use futures_util::Stream;
use rikka_image::ColorScheme;
use rikka_ui::{ImageSource, VectorSource};
use rikka_worker::Observable;

use crate::measuring::{self, Ask, Drawing, Measurement};

#[derive(Debug)]
pub(super) struct Footprints {
    known: HashMap<Arc<Path>, Footprint>,
    scheme: ColorScheme,
    measurer: Option<Measurer>,
    measured: Observable<()>,
}

#[derive(Clone, Copy, Debug)]
enum Footprint {
    Measuring {
        shown: f32,
        drawing: Drawing,
        scheme: ColorScheme,
    },
    Measured {
        scale: f32,
        drawing: Drawing,
        scheme: ColorScheme,
        follows_scheme: bool,
    },
}

#[derive(Debug)]
struct Measurer {
    requests_tx: Sender<Ask>,
    answers: Receiver<Measurement>,
}

impl Footprints {
    #[must_use]
    pub(crate) fn start(scheme: ColorScheme) -> Footprints {
        let measured = Observable::default();
        let (requests_tx, requests) = mpsc::channel();
        let (answered, answers) = mpsc::channel();
        let measurer = match measuring::start(requests, answered, measured.clone()) {
            Ok(()) => Some(Measurer {
                requests_tx,
                answers,
            }),
            Err(error) => {
                tracing::warn!("cannot measure icons, drawing them unscaled: {error}");
                None
            }
        };
        Footprints {
            known: HashMap::new(),
            scheme,
            measurer,
            measured,
        }
    }

    pub(crate) fn changes(&self) -> impl Stream<Item = ()> + Send + 'static + use<> {
        self.measured.changes()
    }

    pub(crate) fn scale(&mut self, picture: &ImageSource) -> f32 {
        let (path, drawing) = match picture {
            ImageSource::Path(path) => (path, Drawing::Raster),
            ImageSource::Vector(vector) => match vector.document() {
                VectorSource::Path(path) => (path, Drawing::Vector),
                VectorSource::Bytes(_) => return 1.0,
            },
            _ => return 1.0,
        };
        self.take_answers();
        if let Some(known) = self.known.get(path) {
            return known.shown();
        }
        self.ask(Arc::clone(path), drawing, 1.0)
    }

    pub(crate) fn wear(&mut self, scheme: ColorScheme) {
        if self.scheme == scheme {
            return;
        }
        self.scheme = scheme;
        self.take_answers();
        let stale: Vec<(Arc<Path>, Drawing, f32)> = self
            .known
            .iter()
            .filter_map(|(path, known)| match *known {
                Footprint::Measured {
                    scale,
                    drawing,
                    scheme: measured,
                    follows_scheme: true,
                } if measured != scheme => Some((Arc::clone(path), drawing, scale)),
                _ => None,
            })
            .collect();
        for (path, drawing, shown) in stale {
            self.ask(path, drawing, shown);
        }
    }

    pub(crate) fn forget(&mut self) {
        self.known.clear();
    }

    fn ask(&mut self, path: Arc<Path>, drawing: Drawing, shown: f32) -> f32 {
        let scheme = self.scheme;
        let asked = self.measurer.as_ref().is_some_and(|measurer| {
            let ask = Ask {
                path: Arc::clone(&path),
                drawing,
                scheme,
            };
            measurer.requests_tx.send(ask).is_ok()
        });
        let known = match asked {
            true => Footprint::Measuring {
                shown,
                drawing,
                scheme,
            },
            false => Footprint::Measured {
                scale: shown,
                drawing,
                scheme,
                follows_scheme: false,
            },
        };
        self.known.insert(path, known);
        shown
    }

    fn take_answers(&mut self) {
        let Some(measurer) = &self.measurer else {
            return;
        };
        for answer in measurer.answers.try_iter() {
            let Some(known) = self.known.get_mut(&answer.path) else {
                continue;
            };
            if let Footprint::Measuring {
                drawing, scheme, ..
            } = *known
                && scheme == answer.scheme
            {
                *known = Footprint::Measured {
                    scale: answer.scale,
                    drawing,
                    scheme,
                    follows_scheme: answer.follows_scheme,
                };
            }
        }
    }
}

impl Footprint {
    fn shown(&self) -> f32 {
        match *self {
            Footprint::Measuring { shown, .. } | Footprint::Measured { scale: shown, .. } => shown,
        }
    }
}
