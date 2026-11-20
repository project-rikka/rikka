use crate::core::video_support::VideoNeed;

#[derive(Clone, Copy, Debug, Default)]
pub struct DeviceRequest {
    #[cfg(unix)]
    pub drm_node: Option<(u32, u32)>,
    pub three_d: bool,
    pub compute: bool,
    pub background: bool,
    pub video: VideoNeed,
    pub prioritized: bool,
    pub transfer: bool,
    pub ray_tracing: bool,
}
