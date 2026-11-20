import rikka.effects.bokeh_blur;

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void prefilter(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size) || push.parameters.radius <= 0.0) {
        return;
    }
    Parameters parameters = *push.parameters;
    uint2 full = rikka_effect_image_size(push.source);
    int2 corner = int2(id.xy) * 2;
    float4 sum = float4(0.0);
    for (int y = 0; y < 2; ++y) {
        for (int x = 0; x < 2; ++x) {
            float4 color = rikka_effect_load_clamped(push.source, corner + int2(x, y), full);
            sum += rikka_bokeh_boosted(parameters.highlights, color);
        }
    }
    rikka_effect_store(push.target, int2(id.xy), sum * 0.25);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void reduce(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size) || push.parameters.radius <= 0.0) {
        return;
    }
    int2 pixel = int2(id.xy);
    rikka_effect_store(push.target, pixel, rikka_pyramid_reduce(push.source, pixel, push.size));
}
