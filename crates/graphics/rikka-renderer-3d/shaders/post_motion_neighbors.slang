import rikka.post;

struct Push {
    uint tiles;
    uint neighbors;
    uint2 size;
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
    float2 best = float2(0.0);
    float best_length = 0.0;
    for (int y = -1; y <= 1; ++y) {
        for (int x = -1; x <= 1; ++x) {
            int2 at = tile + int2(x, y);
            if (any(at < int2(0)) || any(at >= size)) {
                continue;
            }
            float2 velocity = post_fetch(push.tiles, at).xy;
            float span = dot(velocity, velocity);
            if (span > best_length) {
                best_length = span;
                best = velocity;
            }
        }
    }
    post_store(push.neighbors, tile, float4(best, 0.0, 0.0));
}
