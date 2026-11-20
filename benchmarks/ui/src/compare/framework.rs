use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;

use super::error::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Framework {
    Rikka,
    Gpui,
    GpuiFast,
    Bui,
    BuiGpu,
    Egui,
    Iced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Display {
    Wayland,
    X11,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InputSeat {
    Person,
    Agent,
}

impl Framework {
    pub const ALL: [Framework; 7] = [
        Framework::Rikka,
        Framework::Gpui,
        Framework::GpuiFast,
        Framework::Bui,
        Framework::BuiGpu,
        Framework::Egui,
        Framework::Iced,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Framework::Rikka => "rikka",
            Framework::Gpui => "gpui",
            Framework::GpuiFast => "gpui-fast",
            Framework::Bui => "bui",
            Framework::BuiGpu => "bui-gpu",
            Framework::Egui => "egui",
            Framework::Iced => "iced",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Framework> {
        Framework::ALL
            .into_iter()
            .find(|framework| framework.name() == name)
    }

    #[must_use]
    pub(crate) fn display(self) -> Display {
        match self {
            Framework::BuiGpu => Display::X11,
            _ => Display::Wayland,
        }
    }

    #[must_use]
    pub(crate) fn seat(self) -> InputSeat {
        match self {
            Framework::Gpui | Framework::GpuiFast => InputSeat::Agent,
            _ => InputSeat::Person,
        }
    }

    pub fn program(self) -> Result<PathBuf, Error> {
        let variable = self.program_variable();
        let path = std::env::var_os(variable).ok_or(Error::Missing(variable))?;
        std::fs::canonicalize(&path).map_err(|source| Error::Spawn {
            program: PathBuf::from(path),
            source,
        })
    }

    #[must_use]
    pub fn environment(self) -> Vec<(&'static str, OsString)> {
        match self {
            Framework::Bui => vec![("BUI_GPU", "0".into())],
            Framework::BuiGpu => vec![("BUI_GPU", "1".into())],
            _ => Vec::new(),
        }
    }

    #[must_use]
    pub fn version(self) -> &'static str {
        match self {
            Framework::Rikka => "workspace",
            Framework::Gpui => "gpui-pre 0.3.11 (wgpu 30)",
            Framework::GpuiFast => "gpui-fast 0.1.13 (wgpu 30)",
            Framework::Bui => "blenderui 0.5.0, CPU raster, Wayland",
            Framework::BuiGpu => "blenderui 0.5.0, OpenGL 3.3 over GLX, XWayland",
            Framework::Egui => "eframe 0.36.2 (wgpu 30)",
            Framework::Iced => "iced 0.14.0 (wgpu 27)",
        }
    }

    fn program_variable(self) -> &'static str {
        match self {
            Framework::Rikka => "UI_BENCH_RIKKA",
            Framework::Gpui => "UI_BENCH_GPUI",
            Framework::GpuiFast => "UI_BENCH_GPUI_FAST",
            Framework::Bui | Framework::BuiGpu => "UI_BENCH_BUI",
            Framework::Egui => "UI_BENCH_EGUI",
            Framework::Iced => "UI_BENCH_ICED",
        }
    }
}

impl fmt::Display for Framework {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}
