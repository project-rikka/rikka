use windows::Win32::Foundation::E_ACCESSDENIED;
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
use windows::core::HSTRING;

use crate::error::Result;

const MODEL_PREFIX: &str = "Rikka.";
const MODEL_LENGTH: usize = 128;

pub(crate) fn declare(app_name: &str) -> Result<()> {
    match unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) } {
        Ok(()) => {}
        Err(error) if error.code() == E_ACCESSDENIED => {
            tracing::debug!("dpi awareness already declared by the manifest");
        }
        Err(error) => return Err(error.into()),
    }
    unsafe { SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(app_model_id(app_name))) }?;
    Ok(())
}

pub(crate) fn app_model_id(app_name: &str) -> String {
    let product: String = app_name
        .chars()
        .map(
            |letter| match letter.is_ascii_alphanumeric() || letter == '-' {
                true => letter,
                false => '-',
            },
        )
        .collect();
    let mut model = format!("{MODEL_PREFIX}{product}");
    model.truncate(MODEL_LENGTH);
    model
}

pub(crate) struct Ole;

impl Ole {
    pub(crate) fn initialize() -> Result<Ole> {
        unsafe { OleInitialize(None) }?;
        Ok(Ole)
    }
}

impl Drop for Ole {
    fn drop(&mut self) {
        unsafe { OleUninitialize() };
    }
}
