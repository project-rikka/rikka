use std::mem::ManuallyDrop;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use rikka_worker::{EventNotifier, event_fd};
use windows::Win32::Foundation::{E_NOINTERFACE, ERROR_TIMEOUT};
use windows::Win32::Media::Audio::{
    AUDIOCLIENT_ACTIVATION_PARAMS, AUDIOCLIENT_ACTIVATION_PARAMS_0,
    AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK, AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS,
    ActivateAudioInterfaceAsync, IActivateAudioInterfaceAsyncOperation,
    IActivateAudioInterfaceCompletionHandler, IActivateAudioInterfaceCompletionHandler_Impl,
    IAudioClient, PROCESS_LOOPBACK_MODE, VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
};
use windows::Win32::System::Com::StructuredStorage::{
    PROPVARIANT, PROPVARIANT_0, PROPVARIANT_0_0, PROPVARIANT_0_0_0,
};
use windows::Win32::System::Com::{BLOB, IAgileObject, IAgileObject_Impl};
use windows::Win32::System::Variant::VT_BLOB;
use windows::core::{HRESULT, IUnknown, Interface, Ref, implement};

use crate::error::Result;

const ACTIVATION_TIMEOUT: Duration = Duration::from_secs(5);

#[implement(IActivateAudioInterfaceCompletionHandler, IAgileObject)]
struct Completion {
    done: EventNotifier,
    completed: Arc<AtomicBool>,
}

impl IActivateAudioInterfaceCompletionHandler_Impl for Completion_Impl {
    fn ActivateCompleted(
        &self,
        _: Ref<IActivateAudioInterfaceAsyncOperation>,
    ) -> windows::core::Result<()> {
        self.completed.store(true, Ordering::Release);
        if let Err(error) = self.done.notify() {
            tracing::warn!(%error, "failed to signal a finished audio activation");
        }
        Ok(())
    }
}

impl IAgileObject_Impl for Completion_Impl {}

pub(super) fn activate(process: u32, mode: PROCESS_LOOPBACK_MODE) -> Result<IAudioClient> {
    let mut params = AUDIOCLIENT_ACTIVATION_PARAMS {
        ActivationType: AUDIOCLIENT_ACTIVATION_TYPE_PROCESS_LOOPBACK,
        Anonymous: AUDIOCLIENT_ACTIVATION_PARAMS_0 {
            ProcessLoopbackParams: AUDIOCLIENT_PROCESS_LOOPBACK_PARAMS {
                TargetProcessId: process,
                ProcessLoopbackMode: mode,
            },
        },
    };
    let blob = PROPVARIANT {
        Anonymous: PROPVARIANT_0 {
            Anonymous: ManuallyDrop::new(PROPVARIANT_0_0 {
                vt: VT_BLOB,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: PROPVARIANT_0_0_0 {
                    blob: BLOB {
                        cbSize: size_of::<AUDIOCLIENT_ACTIVATION_PARAMS>() as u32,
                        pBlobData: (&raw mut params).cast(),
                    },
                },
            }),
        },
    };
    let (done, finished) = event_fd()?;
    let completed = Arc::new(AtomicBool::new(false));
    let handler: IActivateAudioInterfaceCompletionHandler = Completion {
        done,
        completed: Arc::clone(&completed),
    }
    .into();
    let operation = unsafe {
        ActivateAudioInterfaceAsync(
            VIRTUAL_AUDIO_DEVICE_PROCESS_LOOPBACK,
            &IAudioClient::IID,
            Some(&raw const blob),
            &handler,
        )?
    };
    finished.wait_timeout(ACTIVATION_TIMEOUT)?;
    if !completed.load(Ordering::Acquire) {
        return Err(windows::core::Error::from(ERROR_TIMEOUT.to_hresult()).into());
    }
    let mut outcome = HRESULT(0);
    let mut activated: Option<IUnknown> = None;
    unsafe { operation.GetActivateResult(&mut outcome, &mut activated)? };
    outcome.ok()?;
    let activated = activated.ok_or_else(|| windows::core::Error::from(E_NOINTERFACE))?;
    Ok(activated.cast::<IAudioClient>()?)
}
