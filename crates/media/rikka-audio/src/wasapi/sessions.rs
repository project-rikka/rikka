use std::collections::BTreeMap;
use std::os::windows::io::{FromRawHandle, OwnedHandle};
use std::path::Path;

use rikka_worker::EventNotifier;
use windows::Win32::Foundation::{HANDLE, MAX_PATH, S_OK};
use windows::Win32::Media::Audio::{
    AudioSessionStateExpired, IAudioSessionControl, IAudioSessionControl2, IAudioSessionManager2,
    IAudioSessionNotification, IAudioSessionNotification_Impl, eRender,
};
use windows::Win32::System::Com::{CLSCTX_ALL, CoTaskMemFree};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::core::{Interface, PWSTR, Ref, implement};

use crate::capture::AudioApp;

use super::endpoint::Endpoints;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Session {
    pub(super) process: u32,
    pub(super) key: String,
    pub(super) name: String,
}

pub(super) fn list(endpoints: &Endpoints) -> windows::core::Result<Vec<Session>> {
    let own = std::process::id();
    let mut found = Vec::new();
    for endpoint in endpoints.list(eRender)? {
        let manager: IAudioSessionManager2 = unsafe { endpoint.device.Activate(CLSCTX_ALL, None)? };
        let sessions = unsafe { manager.GetSessionEnumerator()? };
        for index in 0..unsafe { sessions.GetCount()? } {
            let control = unsafe { sessions.GetSession(index)? };
            if let Some(session) = describe(&control, own)?
                && !found
                    .iter()
                    .any(|known: &Session| known.process == session.process)
            {
                found.push(session);
            }
        }
    }
    Ok(found)
}

pub(super) fn apps(endpoints: &Endpoints) -> windows::core::Result<Vec<AudioApp>> {
    let mut apps = BTreeMap::new();
    for session in list(endpoints)? {
        apps.entry(session.key.clone()).or_insert(AudioApp {
            key: session.key,
            name: session.name,
            icon: None,
        });
    }
    Ok(apps.into_values().collect())
}

fn describe(control: &IAudioSessionControl, own: u32) -> windows::core::Result<Option<Session>> {
    if unsafe { control.GetState()? } == AudioSessionStateExpired {
        return Ok(None);
    }
    let control: IAudioSessionControl2 = control.cast()?;
    if unsafe { control.IsSystemSoundsSession() } == S_OK {
        return Ok(None);
    }
    let process = unsafe { control.GetProcessId()? };
    if process == own || process == 0 {
        return Ok(None);
    }
    let Some(key) = image_stem(process) else {
        return Ok(None);
    };
    let display = take(unsafe { control.GetDisplayName()? });
    let name = display
        .filter(|name| !name.is_empty() && !name.starts_with('@'))
        .unwrap_or_else(|| key.clone());
    Ok(Some(Session { process, key, name }))
}

fn image_stem(process: u32) -> Option<String> {
    let handle = match unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process) } {
        Ok(handle) => unsafe { OwnedHandle::from_raw_handle(handle.0) },
        Err(error) => {
            tracing::debug!(%error, process, "cannot inspect an audio process");
            return None;
        }
    };
    let mut path = [0u16; MAX_PATH as usize];
    let mut length = path.len() as u32;
    let queried = unsafe {
        QueryFullProcessImageNameW(
            HANDLE(std::os::windows::io::AsRawHandle::as_raw_handle(&handle)),
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        )
    };
    if let Err(error) = queried {
        tracing::debug!(%error, process, "cannot name an audio process");
        return None;
    }
    let path = String::from_utf16_lossy(&path[..length as usize]);
    Path::new(&path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_lowercase)
}

fn take(text: PWSTR) -> Option<String> {
    if text.is_null() {
        return None;
    }
    let owned = unsafe { text.to_string() }.ok();
    unsafe { CoTaskMemFree(Some(text.as_ptr().cast_const().cast())) };
    owned
}

#[implement(IAudioSessionNotification)]
struct Created {
    changed: EventNotifier,
}

impl IAudioSessionNotification_Impl for Created_Impl {
    fn OnSessionCreated(&self, _: Ref<IAudioSessionControl>) -> windows::core::Result<()> {
        if let Err(error) = self.changed.notify() {
            tracing::warn!(%error, "failed to signal a new audio session");
        }
        Ok(())
    }
}

pub(super) struct SessionWatch {
    registered: Vec<(IAudioSessionManager2, IAudioSessionNotification)>,
}

impl SessionWatch {
    pub(super) fn start(
        endpoints: &Endpoints,
        changed: EventNotifier,
    ) -> windows::core::Result<SessionWatch> {
        let mut watch = SessionWatch {
            registered: Vec::new(),
        };
        for endpoint in endpoints.list(eRender)? {
            let manager: IAudioSessionManager2 =
                unsafe { endpoint.device.Activate(CLSCTX_ALL, None)? };
            let listener: IAudioSessionNotification = Created {
                changed: changed.clone(),
            }
            .into();
            unsafe { manager.GetSessionEnumerator()? };
            unsafe { manager.RegisterSessionNotification(&listener)? };
            watch.registered.push((manager, listener));
        }
        Ok(watch)
    }
}

impl Drop for SessionWatch {
    fn drop(&mut self) {
        for (manager, listener) in &self.registered {
            if let Err(error) = unsafe { manager.UnregisterSessionNotification(listener) } {
                tracing::debug!(%error, "failed to unregister an audio session listener");
            }
        }
    }
}
