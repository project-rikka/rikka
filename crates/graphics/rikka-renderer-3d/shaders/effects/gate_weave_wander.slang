import rikka.effects.gate_weave;

struct Push {
    Parameters* parameters;
    uint target;
    uint2 size;
    float time;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void wander(uint3 id: SV_DispatchThreadID) {
    if (any(id.xy != uint2(0u))) {
        return;
    }
    Parameters parameters = *push.parameters;
    float frame = floor(max(push.time, 0.0) * max(parameters.rate, 1.0));
    float2 offset = parameters.weave * float2(
        rikka_weave_wander(frame, parameters.seed, parameters.jitter),
        rikka_weave_wander(frame, parameters.seed + 1u, parameters.jitter));
    float turn = parameters.rotation * rikka_weave_wander(frame, parameters.seed + 2u, parameters.jitter);
    float stops = parameters.flicker * (rikka_effect_random(uint3(uint(frame), parameters.seed, 0xf1u)) * 2.0 - 1.0);
    rikka_effect_store(push.target, int2(0, 0), float4(offset, turn, stops));
}
