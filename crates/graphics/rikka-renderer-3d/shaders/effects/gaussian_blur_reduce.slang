import rikka.effects.gaussian_blur;

struct Push {
    Parameters* parameters;
    uint image;
    uint size_map;
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
    uint2 full = rikka_effect_image_size(push.image);
    float sigma = rikka_gaussian_sigma(push.parameters, full);
    if (sigma <= 0.0) {
        return;
    }
    uint level = rikka_pyramid_level_of(full, push.size);
    if (rikka_gaussian_deepest(sigma, push.size_map != RIKKA_NONE) < level) {
        return;
    }
    int2 pixel = int2(id.xy);
    rikka_effect_store(push.target, pixel, rikka_pyramid_reduce(push.source, pixel, push.size));
}
