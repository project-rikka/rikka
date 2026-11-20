use std::ffi::c_void;
use std::sync::OnceLock;

use rikka_renderer::{OffscreenTarget, Renderer, Scene, Uploads};
use rikka_types::{Color, Size};
use rikka_ui::{DragIcon, Rendering, State, Ui};
use rikka_ui_host::Resources;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HBITMAP, HDC,
    HGDIOBJ, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HTTRANSPARENT, HWND_TOPMOST, RegisterClassExW,
    SW_HIDE, SWP_NOACTIVATE, SWP_NOSIZE, SWP_SHOWWINDOW, SetWindowPos, ULW_ALPHA,
    UpdateLayeredWindow, WINDOW_EX_STYLE, WM_NCHITTEST, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{PCWSTR, w};

use crate::error::Result;
use crate::win32::window::placement;

const CLASS: PCWSTR = w!("RikkaDragGhost");

pub(crate) struct Ghost {
    ui: Ui,
    shown: bool,
    scene: Scene,
    target: OffscreenTarget,
    resources: Resources,
    uploads: Uploads,
    hotspot: (i32, i32),
    window: GhostWindow,
}

struct GhostWindow(HWND);

impl Drop for GhostWindow {
    fn drop(&mut self) {
        if let Err(error) = unsafe { DestroyWindow(self.0) } {
            tracing::debug!("failed to destroy drag icon: {error}");
        }
    }
}

impl Ghost {
    pub(crate) fn new(renderer: &mut Renderer, icon: DragIcon, scale: f32) -> Result<Ghost> {
        let window = GhostWindow(create_window()?);
        let size = icon.size;
        let hotspot = (
            (icon.hotspot.x * scale).round() as i32,
            (icon.hotspot.y * scale).round() as i32,
        );
        let mut ui = carried(icon);
        ui.set_viewport(size, scale);
        let (width, height) = pixels(size, scale);
        let target = renderer.create_target(width, height)?;
        Ok(Ghost {
            ui,
            shown: true,
            scene: Scene::new(),
            target,
            resources: Resources::new(),
            uploads: renderer.uploads(),
            hotspot,
            window,
        })
    }

    pub(crate) fn follow(&self, cursor: POINT) {
        if !self.shown {
            return;
        }
        if let Err(error) = unsafe {
            SetWindowPos(
                self.window.0,
                Some(HWND_TOPMOST),
                cursor.x - self.hotspot.0,
                cursor.y - self.hotspot.1,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSIZE | SWP_SHOWWINDOW,
            )
        } {
            tracing::debug!("failed to move drag icon: {error}");
        }
    }

    pub(crate) fn hide(&mut self) {
        if !self.shown {
            return;
        }
        self.shown = false;
        placement::show(self.window.0, SW_HIDE);
    }

    pub(crate) fn settle_pictures(&mut self) {
        self.resources.shown(self.ui.shown_images());
        if !self.resources.wants_commit() {
            return;
        }
        let frame = self.resources.commit(&self.uploads);
        self.ui
            .images_arrived(&frame.settled, !frame.updated.is_empty() || frame.repainted);
        for texture in frame.updated {
            self.scene.notify_texture_updated(texture);
        }
    }

    pub(crate) fn loads_pictures(&self) -> bool {
        self.resources.wants_commit()
    }

    pub(crate) fn wants_draw(&self) -> bool {
        self.shown && (self.ui.needs_render() || self.scene.has_damage())
    }

    pub(crate) fn draw(&mut self, renderer: &mut Renderer) {
        self.ui
            .render(Rendering::new(&mut self.scene).resolving(&mut self.resources));
        if let Err(error) = renderer.draw(&mut self.scene, &mut self.target) {
            tracing::warn!("failed to paint drag icon: {error}");
            return;
        }
        let rgba = match renderer.read_pixels(&mut self.target) {
            Ok(rgba) => rgba,
            Err(error) => {
                tracing::warn!("failed to read drag icon: {error}");
                return;
            }
        };
        let size = SIZE {
            cx: self.target.width() as i32,
            cy: self.target.height() as i32,
        };
        if let Err(error) = show_pixels(self.window.0, size, &rgba) {
            tracing::warn!("failed to show drag icon: {error}");
        }
    }
}

impl Drop for Ghost {
    fn drop(&mut self) {
        self.resources.release(&self.uploads);
    }
}

struct MemoryDc(HDC);

impl Drop for MemoryDc {
    fn drop(&mut self) {
        if !unsafe { DeleteDC(self.0) }.as_bool() {
            tracing::debug!("failed to delete drag icon device context");
        }
    }
}

struct Bitmap(HBITMAP);

impl Drop for Bitmap {
    fn drop(&mut self) {
        if !unsafe { DeleteObject(HGDIOBJ(self.0.0)) }.as_bool() {
            tracing::debug!("failed to delete drag icon bitmap");
        }
    }
}

fn show_pixels(window: HWND, size: SIZE, rgba: &[u8]) -> windows::core::Result<()> {
    let header = BITMAPINFOHEADER {
        biSize: size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size.cx,
        biHeight: -size.cy,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let info = BITMAPINFO {
        bmiHeader: header,
        ..Default::default()
    };
    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.is_invalid() {
        return Err(windows::core::Error::from_thread());
    }
    let dc = MemoryDc(dc);
    let mut bits: *mut c_void = std::ptr::null_mut();
    let bitmap =
        Bitmap(unsafe { CreateDIBSection(Some(dc.0), &info, DIB_RGB_COLORS, &mut bits, None, 0) }?);
    let length = size.cx as usize * size.cy as usize * 4;
    let bgra = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u8>(), length) };
    for (to, from) in bgra
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(rgba.as_chunks::<4>().0)
    {
        to.copy_from_slice(&[from[2], from[1], from[0], from[3]]);
    }
    let previous = unsafe { SelectObject(dc.0, HGDIOBJ(bitmap.0.0)) };
    let blend = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: u8::MAX,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let origin = POINT::default();
    let updated = unsafe {
        UpdateLayeredWindow(
            window,
            None,
            None,
            Some(&size),
            Some(dc.0),
            Some(&origin),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        )
    };
    unsafe { SelectObject(dc.0, previous) };
    updated
}

fn carried(icon: DragIcon) -> Ui {
    let theme = icon.theme().clone();
    let mut ui = Ui::new(
        Carried { icon },
        |carried: &Carried| carried.icon.draw(),
        theme,
    );
    ui.set_background(Some(Color::TRANSPARENT));
    ui
}

#[derive(State)]
struct Carried {
    icon: DragIcon,
}

fn pixels(size: Size, scale: f32) -> (u32, u32) {
    let side = |logical: f32| (logical * scale).round().max(1.0) as u32;
    (side(size.width), side(size.height))
}

fn create_window() -> Result<HWND> {
    static REGISTERED: OnceLock<bool> = OnceLock::new();
    let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }?.0);
    let registered = *REGISTERED.get_or_init(|| {
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(ghost_procedure),
            hInstance: instance,
            lpszClassName: CLASS,
            ..WNDCLASSEXW::default()
        };
        (unsafe { RegisterClassExW(&class) }) != 0
    });
    if !registered {
        return Err(windows::core::Error::from_thread().into());
    }
    let extended = WINDOW_EX_STYLE(
        WS_EX_LAYERED.0
            | WS_EX_TOOLWINDOW.0
            | WS_EX_TOPMOST.0
            | WS_EX_NOACTIVATE.0
            | WS_EX_TRANSPARENT.0,
    );
    let window = unsafe {
        CreateWindowExW(
            extended,
            CLASS,
            w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(instance),
            None,
        )
    }?;
    Ok(window)
}

unsafe extern "system" fn ghost_procedure(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
