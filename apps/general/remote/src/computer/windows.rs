use windows::Win32::Foundation::ERROR_MORE_DATA;
use windows::Win32::System::SystemInformation::{
    ComputerNamePhysicalDnsHostname, GetComputerNameExW,
};
use windows::core::PWSTR;

pub(crate) fn name() -> std::io::Result<String> {
    let mut length = 0u32;
    match unsafe { GetComputerNameExW(ComputerNamePhysicalDnsHostname, None, &mut length) } {
        Ok(()) => return Ok(String::new()),
        Err(error) if error.code() == ERROR_MORE_DATA.to_hresult() => {}
        Err(error) => return Err(error.into()),
    }
    let mut buffer = vec![0u16; length as usize];
    unsafe {
        GetComputerNameExW(
            ComputerNamePhysicalDnsHostname,
            Some(PWSTR(buffer.as_mut_ptr())),
            &mut length,
        )
    }?;
    buffer.truncate(length as usize);
    Ok(String::from_utf16_lossy(&buffer))
}
