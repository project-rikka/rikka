import rikka.effects.halation;

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void extract(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size) || push.parameters.amount <= 0.0) {
        return;
    }
    Parameters parameters = *push.parameters;
    uint2 full = rikka_effect_image_size(push.source);
    int2 corner = int2(id.xy) * 2;
    float sum = 0.0;
    for (int y = 0; y < 2; ++y) {
        for (int x = 0; x < 2; ++x) {
            float3 color = max(rikka_effect_straight(rikka_effect_load_clamped(push.source, corner + int2(x, y), full)), float3(0.0));
            sum += color.r * rikka_effect_bright(color.r, parameters.threshold, parameters.knee);
        }
    }
    float exposed = sum * 0.25;
    rikka_effect_store(push.target, int2(id.xy), float4(exposed, exposed, exposed, 1.0));
}
