import rikka.post;
import rikka.post.focus;

struct Push {
    uint coc;
    uint tiles;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

groupshared float nearest[POST_FOCUS_TILE * POST_FOCUS_TILE];
groupshared float widest[POST_FOCUS_TILE * POST_FOCUS_TILE];

[shader("compute")]
[numthreads(POST_FOCUS_TILE, POST_FOCUS_TILE, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID, uint3 group_id: SV_GroupID, uint local_index: SV_GroupIndex) {
    uint index = local_index;
    int2 texel = min(int2(global_id.xy), int2(push.size) - 1);
    float coc = post_fetch(push.coc, texel).r;
    nearest[index] = max(-coc, 0.0);
    widest[index] = abs(coc);
    GroupMemoryBarrierWithGroupSync();
    for (uint stride = uint(POST_FOCUS_TILE * POST_FOCUS_TILE) / 2u; stride > 0u; stride >>= 1u) {
        if (index < stride) {
            nearest[index] = max(nearest[index], nearest[index + stride]);
            widest[index] = max(widest[index], widest[index + stride]);
        }
        GroupMemoryBarrierWithGroupSync();
    }
    if (index == 0u) {
        post_store(push.tiles, int2(group_id.xy), float4(nearest[0], widest[0], 0.0, 0.0));
    }
}
