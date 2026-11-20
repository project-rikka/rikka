import rikka.common;

struct Push {
    uint64_t out_ids;

    int2 origin;
    int2 size;

    uint identity;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 local = int2(global_id.xy);
    if (any(local >= push.size)) {
        return;
    }
    int2 texel = push.origin + local;
    int2 size = int2(rikka_image_size(rikka_identity_images[push.identity], 0));
    uint id = 0u;
    if (all(texel >= int2(0)) && all(texel < size)) {
        id = rikka_identity_images[push.identity].Load(int3(texel, 0)).r;
    }

    id &= RIKKA_IDENTITY_MASK;
    ((RikkaIndexListRef)push.out_ids)[local.y * push.size.x + local.x] = id;
}
