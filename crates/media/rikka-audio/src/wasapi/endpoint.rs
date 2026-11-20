use windows::Win32::Foundation::{ERROR_NOT_FOUND, PROPERTYKEY};
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, EDataFlow, IAudioClient, IMMDevice, IMMDeviceEnumerator,
    IMMNotificationClient, MMDeviceEnumerator, eConsole,
};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToStringAlloc};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree, STGM_READ};
use windows::core::{GUID, PWSTR};

use super::format::{DeviceFormat, MixFormat};

const FRIENDLY_NAME: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
    pid: 14,
};

pub(super) struct Endpoints(IMMDeviceEnumerator);

impl Endpoints {
    pub(super) fn new() -> windows::core::Result<Endpoints> {
        Ok(Endpoints(unsafe {
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?
        }))
    }

    pub(super) fn list(&self, flow: EDataFlow) -> windows::core::Result<Vec<Endpoint>> {
        let collection = unsafe { self.0.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)? };
        let count = unsafe { collection.GetCount()? };
        (0..count)
            .map(|index| Endpoint::new(unsafe { collection.Item(index)? }))
            .collect()
    }

    pub(super) fn default(&self, flow: EDataFlow) -> windows::core::Result<Option<Endpoint>> {
        match unsafe { self.0.GetDefaultAudioEndpoint(flow, eConsole) } {
            Ok(device) => Endpoint::new(device).map(Some),
            Err(error) if error.code() == ERROR_NOT_FOUND.to_hresult() => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(super) fn default_id(&self, flow: EDataFlow) -> windows::core::Result<Option<String>> {
        Ok(self.default(flow)?.map(|endpoint| endpoint.id))
    }

    pub(super) fn find(
        &self,
        flow: EDataFlow,
        name: &str,
    ) -> windows::core::Result<Option<Endpoint>> {
        let endpoints = self.list(flow)?;
        let position = endpoints
            .iter()
            .position(|endpoint| endpoint.name == name)
            .or_else(|| endpoints.iter().position(|endpoint| endpoint.id == name));
        Ok(position.and_then(|position| endpoints.into_iter().nth(position)))
    }

    pub(super) fn is_active(&self, flow: EDataFlow, id: &str) -> windows::core::Result<bool> {
        Ok(self.list(flow)?.iter().any(|endpoint| endpoint.id == id))
    }

    pub(super) fn register(
        &self,
        client: IMMNotificationClient,
    ) -> windows::core::Result<Registration> {
        unsafe { self.0.RegisterEndpointNotificationCallback(&client)? };
        Ok(Registration {
            enumerator: self.0.clone(),
            client,
        })
    }
}

pub(super) struct Registration {
    enumerator: IMMDeviceEnumerator,
    client: IMMNotificationClient,
}

impl Drop for Registration {
    fn drop(&mut self) {
        if let Err(error) = unsafe {
            self.enumerator
                .UnregisterEndpointNotificationCallback(&self.client)
        } {
            tracing::warn!(%error, "failed to unregister the audio device listener");
        }
    }
}

pub(super) struct Endpoint {
    pub(super) device: IMMDevice,
    pub(super) id: String,
    pub(super) name: String,
}

impl Endpoint {
    fn new(device: IMMDevice) -> windows::core::Result<Endpoint> {
        let id = take_string(unsafe { device.GetId()? })?;
        let name = friendly_name(&device)?.unwrap_or_else(|| id.clone());
        Ok(Endpoint { device, id, name })
    }

    pub(super) fn client(&self) -> windows::core::Result<IAudioClient> {
        unsafe { self.device.Activate(CLSCTX_ALL, None) }
    }
}

pub(super) fn mix_format(client: &IAudioClient) -> windows::core::Result<DeviceFormat> {
    let format = unsafe { MixFormat::from_raw(client.GetMixFormat()?) };
    Ok(format.read())
}

fn friendly_name(device: &IMMDevice) -> windows::core::Result<Option<String>> {
    let store = unsafe { device.OpenPropertyStore(STGM_READ)? };
    let mut value = unsafe { store.GetValue(&FRIENDLY_NAME)? };
    let text = unsafe { PropVariantToStringAlloc(&value) };
    if let Err(error) = unsafe { PropVariantClear(&mut value) } {
        tracing::debug!(%error, "failed to clear an audio device property");
    }
    match text {
        Ok(text) => take_string(text).map(|name| Some(name).filter(|name| !name.is_empty())),
        Err(error) => {
            tracing::debug!(%error, "audio device has no friendly name");
            Ok(None)
        }
    }
}

fn take_string(text: PWSTR) -> windows::core::Result<String> {
    let owned = unsafe { text.to_string() };
    unsafe { CoTaskMemFree(Some(text.as_ptr().cast_const().cast())) };
    owned.map_err(|_| windows::core::Error::from(windows::Win32::Foundation::E_INVALIDARG))
}
