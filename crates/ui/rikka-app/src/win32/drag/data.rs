use std::mem::ManuallyDrop;

use windows::Win32::Foundation::{
    DATA_S_SAMEFORMATETC, DV_E_FORMATETC, E_NOTIMPL, OLE_E_ADVISENOTSUPPORTED, S_OK,
};
use windows::Win32::System::Com::{
    DATADIR_GET, FORMATETC, IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC,
    IEnumSTATDATA, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL,
};
use windows::Win32::UI::Shell::SHCreateStdEnumFmtEtc;
use windows::core::{BOOL, HRESULT, Ref, implement};

use crate::win32::drag::formats::{self, Entry};

#[implement(IDataObject)]
pub(crate) struct DataObject {
    entries: Vec<Entry>,
}

impl DataObject {
    pub(crate) fn new(entries: Vec<Entry>) -> DataObject {
        DataObject { entries }
    }

    fn find(&self, wanted: *const FORMATETC) -> Option<&Entry> {
        let wanted = unsafe { wanted.as_ref() }?;
        if wanted.tymed & TYMED_HGLOBAL.0 as u32 == 0 {
            return None;
        }
        self.entries
            .iter()
            .find(|entry| entry.format == wanted.cfFormat)
    }
}

impl IDataObject_Impl for DataObject_Impl {
    fn GetData(&self, wanted: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
        let entry = self
            .find(wanted)
            .ok_or_else(|| windows::core::Error::from_hresult(DV_E_FORMATETC))?;
        let global = formats::global_of(&entry.bytes)?;
        Ok(STGMEDIUM {
            tymed: TYMED_HGLOBAL.0 as u32,
            u: STGMEDIUM_0 { hGlobal: global },
            pUnkForRelease: ManuallyDrop::new(None),
        })
    }

    fn GetDataHere(&self, _: *const FORMATETC, _: *mut STGMEDIUM) -> windows::core::Result<()> {
        Err(windows::core::Error::from_hresult(E_NOTIMPL))
    }

    fn QueryGetData(&self, wanted: *const FORMATETC) -> HRESULT {
        match self.find(wanted) {
            Some(_) => S_OK,
            None => DV_E_FORMATETC,
        }
    }

    fn GetCanonicalFormatEtc(&self, _: *const FORMATETC, out: *mut FORMATETC) -> HRESULT {
        if let Some(out) = unsafe { out.as_mut() } {
            out.ptd = std::ptr::null_mut();
        }
        DATA_S_SAMEFORMATETC
    }

    fn SetData(
        &self,
        _: *const FORMATETC,
        _: *const STGMEDIUM,
        _: BOOL,
    ) -> windows::core::Result<()> {
        Err(windows::core::Error::from_hresult(E_NOTIMPL))
    }

    fn EnumFormatEtc(&self, direction: u32) -> windows::core::Result<IEnumFORMATETC> {
        if direction != DATADIR_GET.0 as u32 {
            return Err(windows::core::Error::from_hresult(E_NOTIMPL));
        }
        let described: Vec<FORMATETC> = self
            .entries
            .iter()
            .map(|entry| formats::describe(entry.format))
            .collect();
        unsafe { SHCreateStdEnumFmtEtc(&described) }
    }

    fn DAdvise(
        &self,
        _: *const FORMATETC,
        _: u32,
        _: Ref<IAdviseSink>,
    ) -> windows::core::Result<u32> {
        Err(windows::core::Error::from_hresult(OLE_E_ADVISENOTSUPPORTED))
    }

    fn DUnadvise(&self, _: u32) -> windows::core::Result<()> {
        Err(windows::core::Error::from_hresult(OLE_E_ADVISENOTSUPPORTED))
    }

    fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> {
        Err(windows::core::Error::from_hresult(OLE_E_ADVISENOTSUPPORTED))
    }
}
