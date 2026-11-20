use rikka_compositor_protocols::ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1;
use rikka_compositor_protocols::ext_image_capture_source_v1::ExtImageCaptureSourceV1;
use rikka_compositor_protocols::ext_image_copy_capture_cursor_session_v1::ExtImageCopyCaptureCursorSessionV1;
use rikka_compositor_protocols::ext_image_copy_capture_frame_v1::ExtImageCopyCaptureFrameV1;
use rikka_compositor_protocols::ext_image_copy_capture_manager_v1::ExtImageCopyCaptureManagerV1;
use rikka_compositor_protocols::ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1;
use rikka_compositor_protocols::ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1;
use rikka_compositor_server::{DisplayHandle, Module};

use super::host::CaptureHost;
use super::protocol::{cursor, frame, manager, session, source};

#[derive(Debug, Clone, Copy, Default)]
pub struct CaptureProtocols;

impl<S: CaptureHost> Module<S> for CaptureProtocols {
    fn install(self, display: &DisplayHandle<S>) {
        display.serve::<ExtOutputImageCaptureSourceManagerV1>(source::output_manager);
        display.serve::<ExtForeignToplevelImageCaptureSourceManagerV1>(source::toplevel_manager);
        display.serve::<ExtImageCaptureSourceV1>(source::request);
        display.serve::<ExtImageCopyCaptureManagerV1>(manager::request);
        display
            .serve::<ExtImageCopyCaptureSessionV1>(session::request)
            .destroyed(session::destroyed);
        display
            .serve::<ExtImageCopyCaptureFrameV1>(frame::request)
            .destroyed(frame::destroyed);
        display
            .serve::<ExtImageCopyCaptureCursorSessionV1>(cursor::request)
            .destroyed(cursor::destroyed);

        display
            .global::<ExtOutputImageCaptureSourceManagerV1>()
            .privileged()
            .publish();
        display
            .global::<ExtForeignToplevelImageCaptureSourceManagerV1>()
            .privileged()
            .publish();
        display
            .global::<ExtImageCopyCaptureManagerV1>()
            .privileged()
            .publish();
    }
}
