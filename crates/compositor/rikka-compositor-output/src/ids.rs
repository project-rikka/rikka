use slotmap::new_key_type;

new_key_type! {
    pub struct CaptureSessionId;
    pub struct CaptureFrameId;
    pub struct CursorSessionId;
    pub struct LeaseDeviceId;
    pub struct LeaseId;
}

new_key_type! {
    pub(crate) struct LeaseRequestId;
}

rikka_wayland_wire::slot_keys!(
    CaptureSessionId,
    CaptureFrameId,
    CursorSessionId,
    LeaseDeviceId,
    LeaseId,
    LeaseRequestId,
);
