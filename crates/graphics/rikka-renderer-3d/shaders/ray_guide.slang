import rikka.denoise;

struct Push {
    RikkaFrameRef frame;
    uint full;
    uint half;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 pixel = global_id.xy;
    if (any(pixel >= push.size)) {
        return;
    }
    float4 guide = rikka_guide_texel(rikka_guide(push.frame, int2(pixel)));
    rikka_storage_rgba16f[push.full][pixel] = guide;
    if (all((pixel & 1u) == 0u)) {
        rikka_storage_rgba16f[push.half][pixel >> 1u] = guide;
    }
}
