import rikka.effects.lens_flare;

[[vk::push_constant]] ConstantBuffer<RikkaBlurPush<Parameters>> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void horizontal(uint3 id: SV_DispatchThreadID) {
    if (push.parameters.amount <= 0.0) {
        return;
    }
    push.spread(id, float2(1.0, 0.0));
}

[shader("compute")]
[numthreads(8, 8, 1)]
void vertical(uint3 id: SV_DispatchThreadID) {
    if (push.parameters.amount <= 0.0) {
        return;
    }
    push.spread(id, float2(0.0, 1.0));
}
