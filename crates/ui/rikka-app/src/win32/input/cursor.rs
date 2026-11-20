use std::collections::HashMap;
use std::ffi::c_void;

use rikka_input::cursor::{CursorPixels, cursor_pixels};
use rikka_types::{CursorImage, CursorShape, ResizeEdge};
use windows::Win32::Graphics::Gdi::{
    BI_BITFIELDS, BITMAPINFO, BITMAPV5HEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject, HBITMAP, HGDIOBJ,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, HCURSOR, HICON, ICONINFO, IDC_APPSTARTING, IDC_ARROW,
    IDC_CROSS, IDC_HAND, IDC_HELP, IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS,
    IDC_SIZENWSE, IDC_SIZEWE, IDC_WAIT, LoadCursorW,
};
use windows::core::PCWSTR;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pointer {
    Shown(HCURSOR),
    Hidden,
}

#[derive(Default)]
pub(crate) struct Cursors {
    system: HashMap<usize, HCURSOR>,
    pictures: HashMap<u64, Picture>,
}

struct Picture(HICON);

impl Drop for Picture {
    fn drop(&mut self) {
        if let Err(error) = unsafe { DestroyIcon(self.0) } {
            tracing::debug!("failed to destroy cursor picture: {error}");
        }
    }
}

impl Cursors {
    pub(crate) fn pointer(&mut self, shape: CursorShape) -> Pointer {
        match shape {
            CursorShape::Hidden => Pointer::Hidden,
            CursorShape::Image(image) => match self.picture(image) {
                Some(cursor) => Pointer::Shown(cursor),
                None => self.system(IDC_ARROW),
            },
            shape => self.system(system_name(shape)),
        }
    }

    pub(crate) fn prune(&mut self, shown: &[CursorShape]) {
        self.pictures.retain(|id, _| {
            let image = CursorImage::new(*id);
            cursor_pixels(image).is_some() || shown.contains(&CursorShape::Image(image))
        });
    }

    fn system(&mut self, name: PCWSTR) -> Pointer {
        let key = name.0 as usize;
        if let Some(cursor) = self.system.get(&key) {
            return Pointer::Shown(*cursor);
        }
        match unsafe { LoadCursorW(None, name) } {
            Ok(cursor) => {
                self.system.insert(key, cursor);
                Pointer::Shown(cursor)
            }
            Err(error) => {
                tracing::warn!("failed to load system cursor: {error}");
                Pointer::Hidden
            }
        }
    }

    fn picture(&mut self, image: CursorImage) -> Option<HCURSOR> {
        if let Some(picture) = self.pictures.get(&image.id()) {
            return Some(HCURSOR(picture.0.0));
        }
        let pixels = cursor_pixels(image)?;
        let icon = match icon_of(&pixels) {
            Ok(icon) => icon,
            Err(error) => {
                tracing::warn!("failed to create cursor picture: {error}");
                return None;
            }
        };
        self.pictures.insert(image.id(), Picture(icon));
        Some(HCURSOR(icon.0))
    }
}

impl std::fmt::Debug for Cursors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cursors")
            .field("pictures", &self.pictures.len())
            .finish_non_exhaustive()
    }
}

fn system_name(shape: CursorShape) -> PCWSTR {
    match shape {
        CursorShape::Default
        | CursorShape::Hidden
        | CursorShape::ContextMenu
        | CursorShape::Alias
        | CursorShape::Copy
        | CursorShape::ZoomIn
        | CursorShape::ZoomOut
        | CursorShape::Image(_) => IDC_ARROW,
        CursorShape::Help => IDC_HELP,
        CursorShape::Pointer | CursorShape::Grab => IDC_HAND,
        CursorShape::Progress => IDC_APPSTARTING,
        CursorShape::Wait => IDC_WAIT,
        CursorShape::Cell | CursorShape::Crosshair => IDC_CROSS,
        CursorShape::Text | CursorShape::VerticalText => IDC_IBEAM,
        CursorShape::Move | CursorShape::Grabbing | CursorShape::AllScroll => IDC_SIZEALL,
        CursorShape::NoDrop | CursorShape::NotAllowed => IDC_NO,
        CursorShape::Resize(ResizeEdge::Top | ResizeEdge::Bottom)
        | CursorShape::ResizeRow
        | CursorShape::ResizeNorthSouth => IDC_SIZENS,
        CursorShape::Resize(ResizeEdge::Left | ResizeEdge::Right)
        | CursorShape::ResizeColumn
        | CursorShape::ResizeEastWest => IDC_SIZEWE,
        CursorShape::Resize(ResizeEdge::TopLeft | ResizeEdge::BottomRight)
        | CursorShape::ResizeNorthWestSouthEast => IDC_SIZENWSE,
        CursorShape::Resize(ResizeEdge::TopRight | ResizeEdge::BottomLeft)
        | CursorShape::ResizeNorthEastSouthWest => IDC_SIZENESW,
    }
}

struct Bitmap(HBITMAP);

impl Drop for Bitmap {
    fn drop(&mut self) {
        if !unsafe { DeleteObject(HGDIOBJ(self.0.0)) }.as_bool() {
            tracing::debug!("failed to delete cursor bitmap");
        }
    }
}

fn icon_of(pixels: &CursorPixels) -> windows::core::Result<HICON> {
    let width = i32::try_from(pixels.width).map_err(|_| invalid())?;
    let height = i32::try_from(pixels.height).map_err(|_| invalid())?;
    let header = BITMAPV5HEADER {
        bV5Size: size_of::<BITMAPV5HEADER>() as u32,
        bV5Width: width,
        bV5Height: -height,
        bV5Planes: 1,
        bV5BitCount: 32,
        bV5Compression: BI_BITFIELDS,
        bV5RedMask: 0x00FF_0000,
        bV5GreenMask: 0x0000_FF00,
        bV5BlueMask: 0x0000_00FF,
        bV5AlphaMask: 0xFF00_0000,
        ..BITMAPV5HEADER::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let color = Bitmap(unsafe {
        CreateDIBSection(
            None,
            (&raw const header).cast::<BITMAPINFO>(),
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )
    }?);
    if bits.is_null() {
        return Err(invalid());
    }
    let straight = straight_bgra(&pixels.premultiplied_bgra);
    unsafe {
        std::ptr::copy_nonoverlapping(straight.as_ptr(), bits.cast::<u8>(), straight.len());
    }
    let mask = Bitmap(unsafe { CreateBitmap(width, height, 1, 1, None) });
    if mask.0.is_invalid() {
        return Err(windows::core::Error::from_thread());
    }
    let info = ICONINFO {
        fIcon: false.into(),
        xHotspot: u32::try_from(pixels.hotspot.0.clamp(0, width - 1)).unwrap_or(0),
        yHotspot: u32::try_from(pixels.hotspot.1.clamp(0, height - 1)).unwrap_or(0),
        hbmMask: mask.0,
        hbmColor: color.0,
    };
    unsafe { CreateIconIndirect(&info) }
}

fn straight_bgra(premultiplied: &[u8]) -> Vec<u8> {
    premultiplied
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|&[blue, green, red, opacity]| {
            let unscale = |channel: u8| match opacity {
                0 => 0,
                _ => ((u16::from(channel) * 255 + u16::from(opacity) / 2) / u16::from(opacity))
                    .min(255) as u8,
            };
            [unscale(blue), unscale(green), unscale(red), opacity]
        })
        .collect()
}

fn invalid() -> windows::core::Error {
    windows::core::Error::from_hresult(windows::Win32::Foundation::E_INVALIDARG)
}
