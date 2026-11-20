use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
use windows::Win32::System::Registry::{
    HKEY, REG_ROUTINE_FLAGS, RRF_RT_REG_BINARY, RRF_RT_REG_DWORD, RegGetValueW,
};
use windows::core::{PCWSTR, Result};

#[derive(Clone, Copy)]
pub(crate) struct Value {
    pub(crate) root: HKEY,
    pub(crate) key: PCWSTR,
    pub(crate) name: PCWSTR,
}

impl Value {
    pub(crate) fn dword(self) -> Result<Option<u32>> {
        let mut value = 0u32;
        let mut size = size_of::<u32>() as u32;
        let found = self.read(RRF_RT_REG_DWORD, Some((&raw mut value).cast()), &mut size)?;
        Ok(found.then_some(value))
    }

    pub(crate) fn binary(self) -> Result<Option<Vec<u8>>> {
        self.bytes(RRF_RT_REG_BINARY)
    }

    fn bytes(self, kind: REG_ROUTINE_FLAGS) -> Result<Option<Vec<u8>>> {
        let mut size = 0u32;
        if !self.read(kind, None, &mut size)? {
            return Ok(None);
        }
        let mut buffer = vec![0u8; size as usize];
        if !self.read(kind, Some(buffer.as_mut_ptr().cast()), &mut size)? {
            return Ok(None);
        }
        buffer.truncate(size as usize);
        Ok(Some(buffer))
    }

    fn read(
        self,
        kind: REG_ROUTINE_FLAGS,
        data: Option<*mut core::ffi::c_void>,
        size: &mut u32,
    ) -> Result<bool> {
        let status =
            unsafe { RegGetValueW(self.root, self.key, self.name, kind, None, data, Some(size)) };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        status.ok()?;
        Ok(true)
    }
}
