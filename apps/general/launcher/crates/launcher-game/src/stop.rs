#[cfg(unix)]
pub(crate) fn terminate(pid: u32) -> std::io::Result<()> {
    use rustix::process::{Pid, Signal, kill_process};
    let pid = i32::try_from(pid)
        .ok()
        .and_then(Pid::from_raw)
        .ok_or_else(|| std::io::Error::other("invalid process id"))?;
    kill_process(pid, Signal::TERM).map_err(std::io::Error::from)
}

#[cfg(windows)]
pub(crate) fn terminate(pid: u32) -> std::io::Result<()> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};

    let process = unsafe { OpenProcess(PROCESS_TERMINATE, false, pid) }
        .map_err(|error| std::io::Error::other(error.message()))?;
    let terminated = unsafe { TerminateProcess(process, 1) };
    let closed = unsafe { CloseHandle(process) };
    terminated.map_err(|error| std::io::Error::other(error.message()))?;
    closed.map_err(|error| std::io::Error::other(error.message()))
}
