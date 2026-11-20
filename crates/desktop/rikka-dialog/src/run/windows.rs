use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt as _;
use std::path::PathBuf;
use std::sync::OnceLock;

use rikka_mime::Database;
use rikka_types::WindowIdentifier;
use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoCreateInstance,
    CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
use windows::Win32::UI::Shell::{
    FOS_ALLOWMULTISELECT, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST,
    FOS_PICKFOLDERS, FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog,
    IShellItem, SHCreateItemFromParsingName, SIGDN_FILESYSPATH,
};
use windows::core::{HSTRING, Interface as _, PCWSTR};

use crate::chooser::{Chooser, ChooserResponse, Purpose};
use crate::error::{Error, Result};
use crate::filter::Filter;

const NAME: &str = "rikka-dialog";

struct Spec {
    name: Vec<u16>,
    globs: Vec<u16>,
    extension: Option<String>,
}

pub(crate) fn run(chooser: &Chooser) -> Result<ChooserResponse> {
    let chooser = chooser.clone();
    let specs = match chooser.directory {
        true => Vec::new(),
        false => chooser.filters.iter().filter_map(spec).collect(),
    };
    std::thread::Builder::new()
        .name(NAME.to_owned())
        .spawn(move || {
            let _apartment = Apartment::enter()?;
            show(&chooser, &specs)
        })
        .map_err(|error| Error::Unavailable(error.to_string()))?
        .join()
        .map_err(|_| Error::Unavailable("file chooser thread panicked".to_owned()))?
}

fn show(chooser: &Chooser, specs: &[Spec]) -> Result<ChooserResponse> {
    let dialog: IFileDialog = match chooser.purpose {
        Purpose::Open => {
            let dialog: IFileOpenDialog = create(&FileOpenDialog)?;
            dialog.cast().map_err(unavailable)?
        }
        Purpose::Save => {
            let dialog: IFileSaveDialog = create(&FileSaveDialog)?;
            dialog.cast().map_err(unavailable)?
        }
    };
    configure(&dialog, chooser, specs).map_err(unavailable)?;
    let owner = chooser.parent.as_ref().and_then(|parent| match parent {
        WindowIdentifier::Win32(window) => Some(HWND(*window as *mut _)),
        _ => None,
    });
    if let Err(error) = unsafe { dialog.Show(owner) } {
        if error.code() == ERROR_CANCELLED.to_hresult() {
            return Ok(ChooserResponse::Cancelled);
        }
        return Err(Error::Unavailable(error.to_string()));
    }
    let files = match chooser.purpose {
        Purpose::Open => {
            let dialog: IFileOpenDialog = dialog.cast().map_err(malformed)?;
            let items = unsafe { dialog.GetResults() }.map_err(malformed)?;
            let count = unsafe { items.GetCount() }.map_err(malformed)?;
            (0..count)
                .map(|index| {
                    let item = unsafe { items.GetItemAt(index) }.map_err(malformed)?;
                    path_of(&item)
                })
                .collect::<Result<Vec<_>>>()?
        }
        Purpose::Save => {
            let item = unsafe { dialog.GetResult() }.map_err(malformed)?;
            vec![path_of(&item)?]
        }
    };
    Ok(ChooserResponse::Files(files))
}

fn configure(dialog: &IFileDialog, chooser: &Chooser, specs: &[Spec]) -> windows::core::Result<()> {
    let mut options = unsafe { dialog.GetOptions() }? | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST;
    match chooser.purpose {
        Purpose::Open => {
            if chooser.multiple {
                options |= FOS_ALLOWMULTISELECT;
            }
            if chooser.directory {
                options |= FOS_PICKFOLDERS;
            }
        }
        Purpose::Save => options |= FOS_OVERWRITEPROMPT,
    }
    unsafe { dialog.SetOptions(options) }?;
    if !chooser.title.is_empty() {
        unsafe { dialog.SetTitle(&HSTRING::from(chooser.title.as_str())) }?;
    }
    if !chooser.accept_label.is_empty() {
        unsafe { dialog.SetOkButtonLabel(&HSTRING::from(chooser.accept_label.as_str())) }?;
    }
    if let Some(folder) = &chooser.folder {
        let item: windows::core::Result<IShellItem> =
            unsafe { SHCreateItemFromParsingName(&HSTRING::from(folder.as_os_str()), None) };
        match item {
            Ok(item) => unsafe { dialog.SetFolder(&item) }?,
            Err(error) => {
                tracing::debug!(%error, folder = %folder.display(), "file chooser folder is unavailable");
            }
        }
    }
    if let Some(name) = &chooser.name {
        unsafe { dialog.SetFileName(&HSTRING::from(name.as_str())) }?;
    }
    if !specs.is_empty() {
        let filters: Vec<COMDLG_FILTERSPEC> = specs
            .iter()
            .map(|spec| COMDLG_FILTERSPEC {
                pszName: PCWSTR(spec.name.as_ptr()),
                pszSpec: PCWSTR(spec.globs.as_ptr()),
            })
            .collect();
        unsafe { dialog.SetFileTypes(&filters) }?;
        unsafe { dialog.SetFileTypeIndex(1) }?;
        if chooser.purpose == Purpose::Save
            && let Some(extension) = &specs[0].extension
        {
            unsafe { dialog.SetDefaultExtension(&HSTRING::from(extension.as_str())) }?;
        }
    }
    Ok(())
}

fn spec(filter: &Filter) -> Option<Spec> {
    let mut globs: Vec<String> = filter.patterns.clone();
    for mime_type in &filter.mime_types {
        let known = database().globs_for(mime_type);
        if known.is_empty() {
            tracing::warn!(%mime_type, filter = %filter.name, "no file name patterns for media type");
        }
        globs.extend(known);
    }
    let mut seen = std::collections::HashSet::new();
    globs.retain(|glob| seen.insert(glob.to_ascii_lowercase()));
    if globs.is_empty() {
        tracing::warn!(filter = %filter.name, "file chooser filter matches no file names");
        return None;
    }
    let extension = globs
        .iter()
        .find_map(|glob| glob.strip_prefix("*."))
        .filter(|extension| !extension.contains(['*', '?', '[']))
        .map(str::to_owned);
    Some(Spec {
        name: wide(&filter.name),
        globs: wide(&globs.join(";")),
        extension,
    })
}

fn database() -> &'static Database {
    static DATABASE: OnceLock<Database> = OnceLock::new();
    DATABASE.get_or_init(Database::load)
}

fn create<T: windows::core::Interface>(class: &windows::core::GUID) -> Result<T> {
    unsafe { CoCreateInstance(class, None, CLSCTX_INPROC_SERVER) }.map_err(unavailable)
}

fn path_of(item: &IShellItem) -> Result<PathBuf> {
    let name = unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) }.map_err(malformed)?;
    let path = PathBuf::from(OsString::from_wide(unsafe { name.as_wide() }));
    unsafe { CoTaskMemFree(Some(name.0.cast_const().cast())) };
    Ok(path)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn unavailable(error: windows::core::Error) -> Error {
    Error::Unavailable(error.to_string())
}

fn malformed(error: windows::core::Error) -> Error {
    Error::Malformed(error.to_string())
}

struct Apartment;

impl Apartment {
    fn enter() -> Result<Apartment> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
            .ok()
            .map_err(unavailable)?;
        Ok(Apartment)
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
