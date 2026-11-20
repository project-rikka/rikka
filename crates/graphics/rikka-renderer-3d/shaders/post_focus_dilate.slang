import rikka.post;
import rikka.post.focus;

struct Push {
    uint tiles;
    uint dilated;
    uint2 size;
    int reach;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 tile = int2(global_id.xy);
    int2 size = int2(push.size);
    if (any(tile >= size)) {
        return;
    }
    float2 own = post_fetch(push.tiles, tile).xy;
    float nearest = own.x;
    for (int y = -push.reach; y <= push.reach; ++y) {
        for (int x = -push.reach; x <= push.reach; ++x) {
            int2 at = tile + int2(x, y);
            if (any(at < int2(0)) || any(at >= size)) {
                continue;
            }
            float other = post_fetch(push.tiles, at).x;
            float gap = float(max(max(abs(x), abs(y)) - 1, 0) * POST_FOCUS_TILE);
            if (other > gap) {
                nearest = max(nearest, other);
            }
        }
    }
    post_store(push.dilated, tile, float4(nearest, max(own.y, nearest), 0.0, 0.0));
}
