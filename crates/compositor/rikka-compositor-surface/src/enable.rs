use std::os::fd::OwnedFd;

use rikka_compositor_protocols::wp_color_manager_v1::WpColorManagerV1;
use rikka_compositor_protocols::wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1;
use rikka_compositor_protocols::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1;
use rikka_compositor_server::{DisplayHandle, Global};

use crate::color::ColorSupport;
use crate::dmabuf::DmabufFeedback;
use crate::surfaces::Surfaces;
use crate::syncobj::{SyncobjDevice, SyncobjError};

impl Surfaces {
    pub fn enable_dmabuf<S: 'static>(
        &mut self,
        display: &DisplayHandle<S>,
        feedback: DmabufFeedback,
    ) -> Global {
        self.set_dmabuf_feedback(feedback);
        display.global::<ZwpLinuxDmabufV1>().publish()
    }

    pub fn enable_syncobj<S: 'static>(
        &mut self,
        display: &DisplayHandle<S>,
        device: OwnedFd,
    ) -> Result<Global, SyncobjError> {
        self.syncobj = Some(SyncobjDevice::open(device)?);
        Ok(display.global::<WpLinuxDrmSyncobjManagerV1>().publish())
    }

    pub fn enable_color_management<S: 'static>(
        &mut self,
        display: &DisplayHandle<S>,
        support: ColorSupport,
    ) -> Global {
        self.color.preferred = Some(self.color.descriptions.describe(support.preferred));
        self.color.support = support;
        display.global::<WpColorManagerV1>().publish()
    }
}
