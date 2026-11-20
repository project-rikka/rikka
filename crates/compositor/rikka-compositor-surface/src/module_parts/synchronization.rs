use rikka_compositor_protocols::wp_commit_timer_v1::WpCommitTimerV1;
use rikka_compositor_protocols::wp_commit_timing_manager_v1::WpCommitTimingManagerV1;
use rikka_compositor_protocols::wp_fifo_manager_v1::WpFifoManagerV1;
use rikka_compositor_protocols::wp_fifo_v1::WpFifoV1;
use rikka_compositor_protocols::wp_linux_drm_syncobj_manager_v1::WpLinuxDrmSyncobjManagerV1;
use rikka_compositor_protocols::wp_linux_drm_syncobj_surface_v1::WpLinuxDrmSyncobjSurfaceV1;
use rikka_compositor_protocols::wp_linux_drm_syncobj_timeline_v1::WpLinuxDrmSyncobjTimelineV1;
use rikka_compositor_protocols::zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1;
use rikka_compositor_protocols::zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1;
use rikka_compositor_protocols::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1;
use rikka_compositor_server::DisplayHandle;

use crate::host::SurfaceHost;
use crate::protocol::{commit_timing, dmabuf, fifo, syncobj};

pub(crate) fn install<S: SurfaceHost>(display: &DisplayHandle<S>) {
    display
        .serve::<ZwpLinuxDmabufV1>(dmabuf::request)
        .bound(dmabuf::bound);
    display.serve::<ZwpLinuxDmabufFeedbackV1>(dmabuf::feedback);
    display
        .serve::<ZwpLinuxBufferParamsV1>(dmabuf::params)
        .destroyed(dmabuf::params_destroyed);
    display.serve::<WpLinuxDrmSyncobjManagerV1>(syncobj::manager);
    display
        .serve::<WpLinuxDrmSyncobjTimelineV1>(syncobj::timeline)
        .destroyed(syncobj::timeline_destroyed);
    display
        .serve::<WpLinuxDrmSyncobjSurfaceV1>(syncobj::surface)
        .destroyed(syncobj::surface_destroyed);
    display.serve::<WpFifoManagerV1>(fifo::manager);
    display
        .serve::<WpFifoV1>(fifo::request)
        .destroyed(fifo::destroyed);
    display.serve::<WpCommitTimingManagerV1>(commit_timing::manager);
    display
        .serve::<WpCommitTimerV1>(commit_timing::request)
        .destroyed(commit_timing::destroyed);
    display.global::<WpFifoManagerV1>().publish();
    display.global::<WpCommitTimingManagerV1>().publish();
}
