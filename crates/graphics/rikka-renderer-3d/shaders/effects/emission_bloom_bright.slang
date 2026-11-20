import rikka.effects.emission_bloom;

struct Push {
    Parameters* parameters;
    uint emission;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void extract(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    rikka_effect_store(
        push.target,
        pixel,
        rikka_emission_bloom_extract(*push.parameters, push.emission, pixel, push.size));
}
