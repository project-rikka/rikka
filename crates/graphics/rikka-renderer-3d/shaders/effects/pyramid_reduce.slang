import rikka.effects.pyramid;

struct Push {
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void reduce(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    int2 pixel = int2(id.xy);
    rikka_effect_store(push.target, pixel, rikka_pyramid_reduce(push.source, pixel, push.size));
}
