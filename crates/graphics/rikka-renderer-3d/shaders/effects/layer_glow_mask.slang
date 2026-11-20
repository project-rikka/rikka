import rikka.effects.layer_glow;

struct Push {
    Parameters* parameters;
    uint layers;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void choose(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    uint2 full = rikka_effect_layers_size(push.layers);
    int2 origin = pixel * 2;
    float covered = 0.0;
    for (int y = 0; y < 2; ++y) {
        for (int x = 0; x < 2; ++x) {
            covered += rikka_glow_chosen(parameters, push.layers, origin + int2(x, y), full) ? 0.25 : 0.0;
        }
    }
    rikka_effect_store_r32(push.target, pixel, covered);
}
