import rikka.post;
import rikka.surface;
import rikka.temporal;

struct Push {
    RikkaFrameRef frame;
    uint velocity;
    uint tiles;
    uint2 size;
    float shutter;
    float limit;
    uint depth_source;
    uint motion_source;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

groupshared float2 longest[256];
groupshared float longest_length[256];

float4 motion_at(RikkaViewRef view, int2 pixel) {
    RikkaScene scene = push.frame.look.scene;
    float2 size = float2(push.size);
    float depth;
    float2 velocity;
    if (push.depth_source != RIKKA_NONE) {
        depth = post_fetch(push.depth_source, pixel).r;
        velocity = post_fetch(push.motion_source, pixel).xy;
    } else {
        depth = rikka_read_depth(scene.depth, pixel);
        velocity = depth <= 0.0
            ? rikka_sky_motion(view, scene.previous_sky_view_projection, (float2(pixel) + 0.5) / size)
            : rikka_read_surface(scene.surface, pixel).velocity;
    }
    float distance = depth <= 0.0 ? POST_SKY_DEPTH : min(rikka_linear_depth(view, depth), POST_SKY_DEPTH);
    float2 pixels = velocity * size * (0.5 * push.shutter);
    float span = length(pixels);
    if (span > push.limit) {
        pixels *= push.limit / span;
    }
    if (any(isnan(pixels))) {
        pixels = float2(0.0);
    }
    return float4(pixels, distance, 0.0);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void compute_main(uint3 group_id: SV_GroupID, uint3 local_id: SV_GroupThreadID, uint local_index: SV_GroupIndex) {
    RikkaViewRef view = push.frame.view;
    uint index = local_index;
    int2 first = int2(group_id.xy) * 32 + int2(local_id.xy) * 2;
    float2 best = float2(0.0);
    float best_length = 0.0;
    for (int y = 0; y < 2; ++y) {
        for (int x = 0; x < 2; ++x) {
            int2 pixel = first + int2(x, y);
            if (any(pixel >= int2(push.size))) {
                continue;
            }
            float4 motion = motion_at(view, pixel);
            post_store(push.velocity, pixel, motion);
            float span = dot(motion.xy, motion.xy);
            if (span > best_length) {
                best_length = span;
                best = motion.xy;
            }
        }
    }
    longest[index] = best;
    longest_length[index] = best_length;
    GroupMemoryBarrierWithGroupSync();
    for (uint stride = 128u; stride > 0u; stride >>= 1u) {
        if (index < stride && longest_length[index + stride] > longest_length[index]) {
            longest_length[index] = longest_length[index + stride];
            longest[index] = longest[index + stride];
        }
        GroupMemoryBarrierWithGroupSync();
    }
    if (index == 0u) {
        post_store(push.tiles, int2(group_id.xy), float4(longest[0], 0.0, 0.0));
    }
}
