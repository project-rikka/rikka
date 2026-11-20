use std::borrow::Cow;
use std::cell::Cell;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_sync::Mutex;
use rikka_types::Size;
use rikka_vector::{Canvas, Pixmap, Transform};

use crate::budget::Budget;
use crate::error::{Error, Result};
use crate::links::folder_of;
use crate::parse::dom::{Dom, Tag};
use crate::parse::entities::expanded;
use crate::parse::media::{ColorScheme, MediaRules};
use crate::raster::Raster;
use crate::render::{Host, draw_group};
use crate::resources::{Decoder, Prefetched, Sources, SvgGlyphs};
use crate::tree::Tree;
use crate::tree::build::build;
use crate::tree::images::picture;

pub const MAX_PIXELS: u64 = 64 * 1024 * 1024;

const MAX_DOCUMENT: u64 = 64 * 1024 * 1024;
const MAX_VARIANTS: usize = 4;
const GZIP: [u8; 2] = [0x1F, 0x8B];
const BYTE_ORDER_MARK: &str = "\u{feff}";

#[derive(Clone)]
pub struct Svg {
    pub(crate) tree: Arc<Tree>,
    responsive: Option<Arc<Responsive>>,
    follows_scheme: bool,
}

struct Responsive {
    source: String,
    rules: MediaRules,
    key: Vec<bool>,
    prefetched: Prefetched,
    svg_glyphs: SvgGlyphs,
    variants: Mutex<Vec<(Vec<bool>, Arc<Tree>)>>,
}

#[derive(Clone, Copy, Default)]
pub struct Loader<'d> {
    decoder: Option<&'d Decoder<'d>>,
    scheme: ColorScheme,
    svg_glyphs: SvgGlyphs,
}

impl<'d> Loader<'d> {
    #[must_use]
    pub fn color_scheme(self, scheme: ColorScheme) -> Loader<'d> {
        Loader { scheme, ..self }
    }

    pub(crate) fn svg_glyphs(self, svg_glyphs: SvgGlyphs) -> Loader<'d> {
        Loader { svg_glyphs, ..self }
    }

    pub fn open(&self, path: impl AsRef<Path>) -> Result<Svg> {
        let path = path.as_ref();
        let data = std::fs::read(path)?;
        self.load(&data, folder_of(path).as_ref())
    }

    pub fn from_bytes(&self, data: &[u8]) -> Result<Svg> {
        self.load(data, None)
    }

    fn load(&self, data: &[u8], folder: Option<&PathBuf>) -> Result<Svg> {
        let text = inflated(data)?;
        let budget = Budget::for_source(text.len());
        let scheme_asked = Cell::new(false);
        let sources = Sources {
            folder,
            decoder: self.decoder,
            prefetched: None,
            scheme: self.scheme,
            svg_glyphs: self.svg_glyphs,
            scheme_asked: &scheme_asked,
            depth: 0,
            budget: &budget,
        };
        let source = markup(&text)?;
        let source = expanded(source)?;
        let dom = Dom::parse(&source, self.scheme, None)?;
        let Some(rules) = dom.media_rules() else {
            let tree = Arc::new(build(&dom, &sources)?);
            return Ok(Svg {
                tree,
                responsive: None,
                follows_scheme: scheme_asked.get(),
            });
        };
        let prefetched = prefetch(&dom, &sources);
        let tree = build(
            &dom,
            &Sources {
                prefetched: Some(&prefetched),
                ..sources
            },
        )?;
        let (width, height) = dom.intrinsic_size();
        Ok(Svg {
            tree: Arc::new(tree),
            follows_scheme: rules.asks_scheme() || scheme_asked.get(),
            responsive: Some(Arc::new(Responsive {
                key: rules.key(width, height),
                source: source.into_owned(),
                rules,
                prefetched,
                svg_glyphs: self.svg_glyphs,
                variants: Mutex::new(Vec::new()),
            })),
        })
    }
}

impl Svg {
    pub fn open(path: impl AsRef<Path>) -> Result<Svg> {
        Loader::default().open(path)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Svg> {
        Loader::default().from_bytes(data)
    }

    #[must_use]
    pub fn decoding<'d>(decoder: &'d Decoder<'d>) -> Loader<'d> {
        Loader {
            decoder: Some(decoder),
            ..Loader::default()
        }
    }

    #[must_use]
    pub fn size(&self) -> Size {
        Size::new(self.tree.width, self.tree.height)
    }

    #[must_use]
    pub fn follows_color_scheme(&self) -> bool {
        self.follows_scheme
    }

    pub fn render(&self, width: u32, height: u32) -> Result<Raster> {
        if width == 0 || height == 0 {
            return Err(Error::InvalidSize { width, height });
        }
        let pixels = u64::from(width) * u64::from(height);
        if pixels > MAX_PIXELS {
            return Err(Error::TooManyPixels {
                width,
                height,
                pixels,
                limit: MAX_PIXELS,
            });
        }
        let tree = self.tree_at(width as f32, height as f32)?;
        let mut canvas = Canvas::new();
        canvas.begin(width, height);
        let fit = fit(&tree, width, height);
        draw_group(&tree.root, &fit, Host::of(&fit), &mut canvas);
        let pixmap = Pixmap::from_premultiplied(width, height, canvas.pixels().to_vec())
            .map_err(|_| Error::InvalidSize { width, height })?;
        Ok(Raster::new(width, height, pixmap.into_straight()))
    }

    fn tree_at(&self, width: f32, height: f32) -> Result<Arc<Tree>> {
        let Some(responsive) = &self.responsive else {
            return Ok(self.tree.clone());
        };
        let key = responsive.rules.key(width, height);
        if key == responsive.key {
            return Ok(self.tree.clone());
        }
        let mut variants = responsive.variants.lock();
        if let Some((_, tree)) = variants.iter().find(|(known, _)| *known == key) {
            return Ok(tree.clone());
        }
        let budget = Budget::for_source(responsive.source.len());
        let scheme_asked = Cell::new(false);
        let sources = Sources {
            folder: None,
            decoder: None,
            prefetched: Some(&responsive.prefetched),
            scheme: responsive.rules.scheme,
            svg_glyphs: responsive.svg_glyphs,
            scheme_asked: &scheme_asked,
            depth: 0,
            budget: &budget,
        };
        let dom = Dom::parse(
            &responsive.source,
            responsive.rules.scheme,
            Some((width, height)),
        )?;
        let tree = Arc::new(build(&dom, &sources)?);
        if variants.len() == MAX_VARIANTS {
            variants.remove(0);
        }
        variants.push((key, tree.clone()));
        Ok(tree)
    }
}

fn fit(tree: &Tree, width: u32, height: u32) -> Transform {
    let (own_width, own_height) = (tree.width, tree.height);
    let scale = (width as f32 / own_width).min(height as f32 / own_height);
    let left = (width as f32 - own_width * scale) / 2.0;
    let top = (height as f32 - own_height * scale) / 2.0;
    Transform::scale(scale, scale).then(Transform::translate(left, top))
}

fn prefetch(dom: &Dom, sources: &Sources<'_>) -> Prefetched {
    let mut prefetched = Prefetched::new();
    for (_, element) in dom.elements() {
        let linked = element.tag == Tag::Image
            || (element.tag == Tag::Primitive && element.name == "feImage");
        let Some(href) = element.attribute("href").filter(|_| linked) else {
            continue;
        };
        if href.trim_start().starts_with('#') || prefetched.contains_key(href) {
            continue;
        }
        prefetched.insert(href.to_owned(), picture(href, sources));
    }
    prefetched
}

pub(crate) fn parse_nested(data: &[u8], sources: &Sources<'_>) -> Result<Tree> {
    let text = inflated(data)?;
    let dom = Dom::parse(&expanded(markup(&text)?)?, sources.scheme, None)?;
    if dom.media_rules().is_some_and(|rules| rules.asks_scheme()) {
        sources.scheme_asked.set(true);
    }
    build(&dom, sources)
}

fn inflated(data: &[u8]) -> Result<Cow<'_, [u8]>> {
    if !data.starts_with(&GZIP) {
        return Ok(Cow::Borrowed(data));
    }
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(data)
        .take(MAX_DOCUMENT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| Error::Malformed(format!("gzip: {error}")))?;
    if bytes.len() as u64 > MAX_DOCUMENT {
        return Err(Error::TooComplex);
    }
    Ok(Cow::Owned(bytes))
}

fn markup(data: &[u8]) -> Result<&str> {
    let text = std::str::from_utf8(data)
        .map_err(|error| Error::Malformed(format!("not UTF-8: {error}")))?;
    Ok(text.strip_prefix(BYTE_ORDER_MARK).unwrap_or(text))
}

impl std::fmt::Debug for Svg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Svg")
            .field("size", &self.size())
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for Loader<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Loader")
            .field("decodes", &self.decoder.is_some())
            .field("scheme", &self.scheme)
            .finish()
    }
}
