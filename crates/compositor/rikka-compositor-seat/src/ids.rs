use slotmap::new_key_type;

new_key_type! {
    pub struct SeatId;
    pub struct VirtualKeyboardId;
    pub struct TabletId;
    pub struct ToolId;
    pub struct PadId;
}

new_key_type! {
    pub(crate) struct NotificationId;
    pub(crate) struct VirtualPointerId;
}

rikka_wayland_wire::slot_keys!(
    SeatId,
    VirtualKeyboardId,
    TabletId,
    ToolId,
    PadId,
    NotificationId,
    VirtualPointerId,
);
