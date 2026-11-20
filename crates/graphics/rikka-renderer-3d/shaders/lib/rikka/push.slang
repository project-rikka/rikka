__exported import rikka.common;

struct RikkaPush {
    RikkaFrameRef frame;
    uint slot;
    uint flags;
};

[[vk::push_constant]] ConstantBuffer<RikkaPush> rikka_push;

RikkaViewRef rikka_view() {
    return rikka_push.frame.view;
}
