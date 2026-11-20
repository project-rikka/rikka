use std::io;
use std::os::windows::io::{
    AsHandle as _, AsRawHandle as _, BorrowedHandle, FromRawHandle as _, OwnedHandle,
};
use std::time::Duration;

use rikka_worker::EventFd;
use windows::Win32::Foundation::{
    ERROR_IO_PENDING, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE, HANDLE,
    WAIT_OBJECT_0,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, FILE_SHARE_NONE,
    OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile, SECURITY_IDENTIFICATION, SECURITY_SQOS_PRESENT,
    WriteFile,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    WaitNamedPipeW,
};
use windows::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject};
use windows::core::HSTRING;

use super::identity::Descriptor;

const BUFFER: u32 = 64 * 1024;

const CONNECT_ATTEMPTS: usize = 3;

const BUSY_WAIT_MILLISECONDS: u32 = 2000;

pub(super) enum Creation {
    First(Server),
    Taken,
}

pub(super) struct Server {
    pipe: OwnedHandle,
    event: OwnedHandle,
}

impl Server {
    pub(super) fn create(name: &HSTRING, descriptor: &Descriptor) -> io::Result<Creation> {
        let attributes = descriptor.attributes();
        let handle = unsafe {
            CreateNamedPipeW(
                name,
                PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                BUFFER,
                BUFFER,
                0,
                Some(&attributes),
            )
        };
        if handle.is_invalid() {
            let error = io::Error::last_os_error();
            return match error.kind() {
                io::ErrorKind::PermissionDenied => Ok(Creation::Taken),
                _ => Err(error),
            };
        }
        let pipe = unsafe { OwnedHandle::from_raw_handle(handle.0) };
        let event = unsafe { CreateEventW(None, true, false, None) }?;
        Ok(Creation::First(Server {
            pipe,
            event: unsafe { OwnedHandle::from_raw_handle(event.0) },
        }))
    }

    pub(super) fn accept(&self, wake: &EventFd) -> io::Result<Option<()>> {
        self.complete(wake, None, |pipe, overlapped| unsafe {
            ConnectNamedPipe(pipe, Some(overlapped))
        })
        .map(|done| done.map(drop))
    }

    pub(super) fn read_exact(
        &self,
        wake: &EventFd,
        timeout: Duration,
        buffer: &mut [u8],
    ) -> io::Result<Option<()>> {
        let mut filled = 0;
        while filled < buffer.len() {
            let rest = &mut buffer[filled..];
            let read = self.complete(wake, Some(timeout), |pipe, overlapped| unsafe {
                ReadFile(pipe, Some(rest), None, Some(overlapped))
            })?;
            match read {
                None => return Ok(None),
                Some(0) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
                Some(count) => filled += count as usize,
            }
        }
        Ok(Some(()))
    }

    pub(super) fn write_all(
        &self,
        wake: &EventFd,
        timeout: Duration,
        bytes: &[u8],
    ) -> io::Result<Option<()>> {
        let mut written = 0;
        while written < bytes.len() {
            let rest = &bytes[written..];
            let wrote = self.complete(wake, Some(timeout), |pipe, overlapped| unsafe {
                WriteFile(pipe, Some(rest), None, Some(overlapped))
            })?;
            match wrote {
                None => return Ok(None),
                Some(0) => return Err(io::Error::from(io::ErrorKind::WriteZero)),
                Some(count) => written += count as usize,
            }
        }
        Ok(Some(()))
    }

    pub(super) fn disconnect(&self) {
        if let Err(error) = unsafe { DisconnectNamedPipe(self.raw()) } {
            tracing::debug!(%error, "failed to disconnect instance client");
        }
    }

    fn raw(&self) -> HANDLE {
        HANDLE(self.pipe.as_raw_handle())
    }

    fn complete(
        &self,
        wake: &EventFd,
        timeout: Option<Duration>,
        start: impl FnOnce(HANDLE, *mut OVERLAPPED) -> windows::core::Result<()>,
    ) -> io::Result<Option<u32>> {
        unsafe { ResetEvent(HANDLE(self.event.as_raw_handle())) }?;
        let mut overlapped = OVERLAPPED {
            hEvent: HANDLE(self.event.as_raw_handle()),
            ..OVERLAPPED::default()
        };
        match start(self.raw(), &mut overlapped) {
            Ok(()) => {}
            Err(error) if error.code() == ERROR_PIPE_CONNECTED.to_hresult() => return Ok(Some(0)),
            Err(error) if error.code() == ERROR_IO_PENDING.to_hresult() => {
                let [done] = wake.wait_signaled([self.event.as_handle()], timeout)?;
                if !done {
                    if let Err(error) = unsafe { CancelIoEx(self.raw(), Some(&overlapped)) } {
                        tracing::debug!(%error, "failed to cancel instance pipe operation");
                    }
                    let mut transferred = 0u32;
                    let _ = unsafe {
                        GetOverlappedResult(self.raw(), &overlapped, &mut transferred, true)
                    };
                    return match is_signaled(wake.as_handle()) {
                        true => Ok(None),
                        false => Err(io::Error::from(io::ErrorKind::TimedOut)),
                    };
                }
            }
            Err(error) => return Err(error.into()),
        }
        let mut transferred = 0u32;
        unsafe { GetOverlappedResult(self.raw(), &overlapped, &mut transferred, true) }?;
        Ok(Some(transferred))
    }
}

fn is_signaled(handle: BorrowedHandle<'_>) -> bool {
    let outcome = unsafe { WaitForSingleObject(HANDLE(handle.as_raw_handle()), 0) };
    outcome == WAIT_OBJECT_0
}

pub(super) struct Client {
    pipe: OwnedHandle,
}

impl Client {
    pub(super) fn connect(name: &HSTRING) -> io::Result<Client> {
        let mut attempts = 0;
        loop {
            attempts += 1;
            let opened = unsafe {
                CreateFileW(
                    name,
                    (GENERIC_READ | GENERIC_WRITE).0,
                    FILE_SHARE_NONE,
                    None,
                    OPEN_EXISTING,
                    SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    None,
                )
            };
            match opened {
                Ok(handle) => {
                    return Ok(Client {
                        pipe: unsafe { OwnedHandle::from_raw_handle(handle.0) },
                    });
                }
                Err(error)
                    if error.code() == ERROR_PIPE_BUSY.to_hresult()
                        && attempts < CONNECT_ATTEMPTS =>
                {
                    let _ = unsafe { WaitNamedPipeW(name, BUSY_WAIT_MILLISECONDS) };
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub(super) fn handle(&self) -> BorrowedHandle<'_> {
        self.pipe.as_handle()
    }

    pub(super) fn write_all(&self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() {
            let mut written = 0u32;
            unsafe { WriteFile(self.raw(), Some(bytes), Some(&mut written), None) }?;
            if written == 0 {
                return Err(io::Error::from(io::ErrorKind::WriteZero));
            }
            bytes = &bytes[written as usize..];
        }
        Ok(())
    }

    pub(super) fn read_exact(&self, mut buffer: &mut [u8]) -> io::Result<()> {
        while !buffer.is_empty() {
            let mut read = 0u32;
            unsafe { ReadFile(self.raw(), Some(buffer), Some(&mut read), None) }?;
            if read == 0 {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
            }
            buffer = &mut buffer[read as usize..];
        }
        Ok(())
    }

    fn raw(&self) -> HANDLE {
        HANDLE(self.pipe.as_raw_handle())
    }
}
