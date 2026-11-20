use rikka_worker::EventNotifier;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, EDataFlow, ERole, IMMNotificationClient, IMMNotificationClient_Impl,
};
use windows::core::{PCWSTR, implement};

#[implement(IMMNotificationClient)]
struct Notifier {
    changed: EventNotifier,
}

impl Notifier {
    fn ring(&self) -> windows::core::Result<()> {
        if let Err(error) = self.changed.notify() {
            tracing::warn!(%error, "failed to signal an audio device change");
        }
        Ok(())
    }
}

impl IMMNotificationClient_Impl for Notifier_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        self.ring()
    }

    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.ring()
    }

    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.ring()
    }

    fn OnDefaultDeviceChanged(
        &self,
        _: EDataFlow,
        _: ERole,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        self.ring()
    }

    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}

pub(super) fn listener(changed: EventNotifier) -> IMMNotificationClient {
    Notifier { changed }.into()
}
