__exported import rikka.emitter;

struct RikkaEmitterPush {
    RikkaFrameRef frame;
    RikkaEmitterRef emitters;
    RikkaEmitterKeyRef keys;
    RikkaEmitterParticleRef particles;
    uint emitter;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<RikkaEmitterPush> rikka_emitter_push;

RikkaEmitterRef rikka_pushed_emitter() {
    return rikka_emitter_push.emitters + rikka_emitter_push.emitter;
}
