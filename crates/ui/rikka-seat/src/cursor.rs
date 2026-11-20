use std::collections::HashMap;

use rikka_types::{CursorImage, CursorShape, ResizeEdge};
use rikka_wayland_client::wl_compositor::WlCompositor;
use rikka_wayland_client::wl_pointer::WlPointer;
use rikka_wayland_client::wl_shm::WlShm;
use rikka_wayland_client::wl_surface::WlSurface;
use rikka_wayland_client::wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1};
use rikka_wayland_client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;
use rikka_wayland_client::zwp_tablet_tool_v2::ZwpTabletToolV2;
use rikka_xcursor::Theme;

use crate::picture::{self, Prepared, Uploaded};
use rikka_input::cursor::cursor_pixels;

const THEME_SIZE: u32 = 24;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Owner<'a> {
    Pointer(&'a WlPointer),
    Tool(&'a ZwpTabletToolV2),
}

impl Owner<'_> {
    fn set_cursor(self, serial: u32, surface: Option<&WlSurface>, x: i32, y: i32) {
        match self {
            Owner::Pointer(pointer) => pointer.set_cursor(serial, surface, x, y),
            Owner::Tool(tool) => tool.set_cursor(serial, surface, x, y),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Cursors {
    pub(crate) source: Source,
    pub(crate) pictures: Option<Pictures>,
}

#[derive(Debug)]
pub(crate) enum Source {
    Shapes(WpCursorShapeManagerV1),
    Themed(Box<Themes>),
    Absent,
}

pub(crate) struct Pictures {
    shm: WlShm,
    compositor: WlCompositor,
    uploaded: HashMap<u64, Uploaded>,
}

impl Pictures {
    pub(crate) fn new(shm: WlShm, compositor: WlCompositor) -> Pictures {
        Pictures {
            shm,
            compositor,
            uploaded: HashMap::new(),
        }
    }

    fn show(
        &mut self,
        image: CursorImage,
        owner: Owner<'_>,
        surface: &WlSurface,
        serial: u32,
        scale: u32,
    ) -> bool {
        self.uploaded
            .retain(|id, _| cursor_pixels(CursorImage::new(*id)).is_some());
        let Some(prepared) = picture::prepared(image) else {
            return false;
        };
        if !self.uploaded.contains_key(&image.id()) {
            let Some(uploaded) = Uploaded::new(&self.shm, &prepared) else {
                return false;
            };
            self.uploaded.insert(image.id(), uploaded);
        }
        let Some(uploaded) = self.uploaded.get(&image.id()) else {
            return false;
        };
        let scale = i32::try_from(scale.max(1)).unwrap_or(1);
        let scale = match prepared.width % scale == 0 && prepared.height % scale == 0 {
            true => scale,
            false => 1,
        };
        surface.set_buffer_scale(scale);
        surface.attach(Some(&uploaded.buffer), 0, 0);
        surface.damage_buffer(0, 0, prepared.width, prepared.height);
        surface.commit();
        owner.set_cursor(
            serial,
            Some(surface),
            prepared.hotspot.0 / scale,
            prepared.hotspot.1 / scale,
        );
        true
    }
}

impl std::fmt::Debug for Pictures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pictures")
            .field("uploaded", &self.uploaded.len())
            .finish_non_exhaustive()
    }
}

pub(crate) struct Themes {
    shm: WlShm,
    compositor: WlCompositor,
    theme: Theme,
    name: String,
    size: u32,
    loaded: HashMap<(&'static str, u32), Option<Themed>>,
}

struct Themed {
    picture: Prepared,
    uploaded: Uploaded,
}

#[derive(Debug)]
pub(crate) struct Cursor {
    device: Device,
    picture: Option<WlSurface>,
}

#[derive(Debug)]
enum Device {
    Shaped(WpCursorShapeDeviceV1),
    Themed(WlSurface),
    Bare,
}

impl Themes {
    pub(crate) fn new(shm: WlShm, compositor: WlCompositor) -> Themes {
        let name = std::env::var("XCURSOR_THEME")
            .ok()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "default".to_owned());
        let size = std::env::var("XCURSOR_SIZE")
            .ok()
            .and_then(|size| size.parse().ok())
            .filter(|size| *size > 0)
            .unwrap_or(THEME_SIZE);
        Themes {
            shm,
            compositor,
            theme: Theme::load(&name),
            name,
            size,
            loaded: HashMap::new(),
        }
    }

    fn image(&mut self, shape: CursorShape, scale: u32) -> Option<&Themed> {
        let name = names(shape)
            .iter()
            .chain(names(CursorShape::Default))
            .copied()
            .find(|name| self.load(name, scale))?;
        self.loaded.get(&(name, scale))?.as_ref()
    }

    fn load(&mut self, name: &'static str, scale: u32) -> bool {
        let Themes {
            shm,
            theme,
            size,
            loaded,
            ..
        } = self;
        loaded
            .entry((name, scale))
            .or_insert_with(|| {
                let picture = read(theme, name, size.saturating_mul(scale))?;
                let uploaded = Uploaded::new(shm, &picture)?;
                Some(Themed { picture, uploaded })
            })
            .is_some()
    }
}

fn read(theme: &Theme, name: &str, size: u32) -> Option<Prepared> {
    let path = theme.find(name)?;
    let cursor = rikka_xcursor::Cursor::read(&path, size)
        .inspect_err(|error| tracing::debug!("cannot read cursor {}: {error}", path.display()))
        .ok()?;
    let frame = cursor.frames.into_iter().next()?;
    Prepared::from_bgra(frame.width, frame.height, frame.hotspot, frame.bgra)
}

impl std::fmt::Debug for Themes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Themes")
            .field("name", &self.name)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl Cursors {
    pub(crate) fn device(&self, owner: Owner<'_>) -> Cursor {
        let device = match &self.source {
            Source::Shapes(manager) => Device::Shaped(match owner {
                Owner::Pointer(pointer) => manager.get_pointer(pointer),
                Owner::Tool(tool) => manager.get_tablet_tool_v2(tool),
            }),
            Source::Themed(themes) => Device::Themed(themes.compositor.create_surface()),
            Source::Absent => Device::Bare,
        };
        let picture = self
            .pictures
            .as_ref()
            .map(|pictures| pictures.compositor.create_surface());
        Cursor { device, picture }
    }
}

impl Cursor {
    pub(crate) fn release(self) {
        match self.device {
            Device::Shaped(device) => device.destroy(),
            Device::Themed(surface) => surface.destroy(),
            Device::Bare => {}
        }
        if let Some(picture) = self.picture {
            picture.destroy();
        }
    }

    pub(crate) fn show(
        &self,
        owner: Owner<'_>,
        serial: u32,
        shape: CursorShape,
        scale: u32,
        cursors: &mut Cursors,
    ) {
        if shape == CursorShape::Hidden {
            owner.set_cursor(serial, None, 0, 0);
            return;
        }
        let shape = match shape {
            CursorShape::Image(image) => {
                if let (Some(surface), Some(pictures)) = (&self.picture, &mut cursors.pictures)
                    && pictures.show(image, owner, surface, serial, scale)
                {
                    return;
                }
                CursorShape::Default
            }
            other => other,
        };
        match (&self.device, &mut cursors.source) {
            (Device::Shaped(device), _) => device.set_shape(serial, wire_shape(shape)),
            (Device::Themed(surface), Source::Themed(themes)) => {
                themed(owner, surface, serial, shape, scale.max(1), themes);
            }
            _ => {}
        }
    }
}

fn themed(
    owner: Owner<'_>,
    surface: &WlSurface,
    serial: u32,
    shape: CursorShape,
    scale: u32,
    themes: &mut Themes,
) {
    let Some(image) = themes.image(shape, scale) else {
        tracing::debug!("cursor theme has no image for {shape:?}");
        return;
    };
    let scale = i32::try_from(scale).unwrap_or(1);
    surface.set_buffer_scale(scale);
    surface.attach(Some(&image.uploaded.buffer), 0, 0);
    surface.damage_buffer(0, 0, image.picture.width, image.picture.height);
    surface.commit();
    owner.set_cursor(
        serial,
        Some(surface),
        image.picture.hotspot.0 / scale,
        image.picture.hotspot.1 / scale,
    );
}

fn names(shape: CursorShape) -> &'static [&'static str] {
    match shape {
        CursorShape::Default | CursorShape::Hidden | CursorShape::Image(_) => {
            &["default", "left_ptr"]
        }
        CursorShape::ContextMenu => &["context-menu"],
        CursorShape::Help => &["help", "question_arrow"],
        CursorShape::Pointer => &["pointer", "hand2", "hand1"],
        CursorShape::Progress => &["progress", "left_ptr_watch"],
        CursorShape::Wait => &["wait", "watch"],
        CursorShape::Cell => &["cell", "plus"],
        CursorShape::Crosshair => &["crosshair", "cross"],
        CursorShape::Text => &["text", "xterm"],
        CursorShape::VerticalText => &["vertical-text"],
        CursorShape::Alias => &["alias", "dnd-link"],
        CursorShape::Copy => &["copy", "dnd-copy"],
        CursorShape::Move => &["move", "fleur"],
        CursorShape::NoDrop => &["no-drop", "dnd-none"],
        CursorShape::NotAllowed => &["not-allowed", "crossed_circle"],
        CursorShape::Grab => &["grab", "openhand", "hand1"],
        CursorShape::Grabbing => &["grabbing", "closedhand"],
        CursorShape::AllScroll => &["all-scroll", "fleur"],
        CursorShape::Resize(edge) => match edge {
            ResizeEdge::Top => &["n-resize", "top_side"],
            ResizeEdge::Bottom => &["s-resize", "bottom_side"],
            ResizeEdge::Left => &["w-resize", "left_side"],
            ResizeEdge::Right => &["e-resize", "right_side"],
            ResizeEdge::TopLeft => &["nw-resize", "top_left_corner"],
            ResizeEdge::TopRight => &["ne-resize", "top_right_corner"],
            ResizeEdge::BottomLeft => &["sw-resize", "bottom_left_corner"],
            ResizeEdge::BottomRight => &["se-resize", "bottom_right_corner"],
        },
        CursorShape::ResizeColumn => &["col-resize", "sb_h_double_arrow"],
        CursorShape::ResizeRow => &["row-resize", "sb_v_double_arrow"],
        CursorShape::ResizeEastWest => &["ew-resize", "sb_h_double_arrow"],
        CursorShape::ResizeNorthSouth => &["ns-resize", "sb_v_double_arrow"],
        CursorShape::ResizeNorthEastSouthWest => &["nesw-resize", "fd_double_arrow"],
        CursorShape::ResizeNorthWestSouthEast => &["nwse-resize", "bd_double_arrow"],
        CursorShape::ZoomIn => &["zoom-in"],
        CursorShape::ZoomOut => &["zoom-out"],
    }
}

fn wire_shape(shape: CursorShape) -> Shape {
    match shape {
        CursorShape::Default | CursorShape::Hidden | CursorShape::Image(_) => Shape::Default,
        CursorShape::ContextMenu => Shape::ContextMenu,
        CursorShape::Help => Shape::Help,
        CursorShape::Pointer => Shape::Pointer,
        CursorShape::Progress => Shape::Progress,
        CursorShape::Wait => Shape::Wait,
        CursorShape::Cell => Shape::Cell,
        CursorShape::Crosshair => Shape::Crosshair,
        CursorShape::Text => Shape::Text,
        CursorShape::VerticalText => Shape::VerticalText,
        CursorShape::Alias => Shape::Alias,
        CursorShape::Copy => Shape::Copy,
        CursorShape::Move => Shape::Move,
        CursorShape::NoDrop => Shape::NoDrop,
        CursorShape::NotAllowed => Shape::NotAllowed,
        CursorShape::Grab => Shape::Grab,
        CursorShape::Grabbing => Shape::Grabbing,
        CursorShape::AllScroll => Shape::AllScroll,
        CursorShape::Resize(edge) => match edge {
            ResizeEdge::Top => Shape::NResize,
            ResizeEdge::Bottom => Shape::SResize,
            ResizeEdge::Left => Shape::WResize,
            ResizeEdge::Right => Shape::EResize,
            ResizeEdge::TopLeft => Shape::NwResize,
            ResizeEdge::TopRight => Shape::NeResize,
            ResizeEdge::BottomLeft => Shape::SwResize,
            ResizeEdge::BottomRight => Shape::SeResize,
        },
        CursorShape::ResizeColumn => Shape::ColResize,
        CursorShape::ResizeRow => Shape::RowResize,
        CursorShape::ResizeEastWest => Shape::EwResize,
        CursorShape::ResizeNorthSouth => Shape::NsResize,
        CursorShape::ResizeNorthEastSouthWest => Shape::NeswResize,
        CursorShape::ResizeNorthWestSouthEast => Shape::NwseResize,
        CursorShape::ZoomIn => Shape::ZoomIn,
        CursorShape::ZoomOut => Shape::ZoomOut,
    }
}
