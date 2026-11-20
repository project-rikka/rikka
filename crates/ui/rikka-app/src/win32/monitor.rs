use rikka_picture::RefreshRate;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    DEVMODEW, ENUM_CURRENT_SETTINGS, EnumDisplaySettingsW, GetMonitorInfoW, HMONITOR,
    MONITOR_DEFAULTTONEAREST, MONITORINFO, MONITORINFOEXW, MonitorFromWindow,
};
use windows::core::PCWSTR;

const MILLIHERTZ_PER_HERTZ: u32 = 1000;
const UNSPECIFIED_FREQUENCY: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Display(HMONITOR);

impl Display {
    pub(crate) fn of(window: HWND) -> Display {
        Display(unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) })
    }

    pub(crate) fn work_area(self) -> Option<RECT> {
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..MONITORINFO::default()
        };
        unsafe { GetMonitorInfoW(self.0, &mut info) }
            .as_bool()
            .then_some(info.rcWork)
    }

    pub(crate) fn refresh(self) -> Option<RefreshRate> {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if !unsafe { GetMonitorInfoW(self.0, (&raw mut info).cast()) }.as_bool() {
            return None;
        }
        let mut mode = DEVMODEW {
            dmSize: size_of::<DEVMODEW>() as u16,
            ..DEVMODEW::default()
        };
        let read = unsafe {
            EnumDisplaySettingsW(
                PCWSTR(info.szDevice.as_ptr()),
                ENUM_CURRENT_SETTINGS,
                &mut mode,
            )
        };
        if !read.as_bool() || mode.dmDisplayFrequency <= UNSPECIFIED_FREQUENCY {
            return None;
        }
        RefreshRate::from_millihertz(mode.dmDisplayFrequency.saturating_mul(MILLIHERTZ_PER_HERTZ))
    }
}

pub(crate) fn publish(displays: impl IntoIterator<Item = Display>) {
    if let Some(fastest) = displays.into_iter().filter_map(Display::refresh).max() {
        fastest.publish();
    }
}
