import rikka.ray;
import rikka.denoise;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint samples;
    uint2 size;
    uint4 lights;
    uint tiles;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint TILE_LEVELS = 1023u;

groupshared uint tile_low[RIKKA_TRACED_LIGHTS];
groupshared uint tile_high[RIKKA_TRACED_LIGHTS];

float4 visible_at(RikkaFrameRef frame, uint2 pixel) {
    RikkaGuide guide = rikka_guide(frame, int2(pixel));
    if (!guide.covered) {
        return float4(1.0);
    }
    return rikka_ray_shadows(
        frame, push.lights, push.samples, rikka_world_of(frame, int2(pixel)), guide.normal, guide.depth, pixel);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID, uint3 local: SV_GroupThreadID, uint3 group: SV_GroupID) {
    uint flat = local.y * RIKKA_TILE + local.x;
    if (flat < RIKKA_TRACED_LIGHTS) {
        tile_low[flat] = TILE_LEVELS;
        tile_high[flat] = 0u;
    }
    GroupMemoryBarrierWithGroupSync();
    uint2 pixel = global_id.xy;
    if (all(pixel < push.size)) {
        float4 visibility = visible_at(push.frame, pixel);
        rikka_storage_rgba16f[push.target][pixel] = visibility;
        [unroll]
        for (uint channel = 0u; channel < RIKKA_TRACED_LIGHTS; ++channel) {
            uint level = uint(saturate(visibility[channel]) * float(TILE_LEVELS) + 0.5);
            InterlockedMin(tile_low[channel], level);
            InterlockedMax(tile_high[channel], level);
        }
    }
    GroupMemoryBarrierWithGroupSync();
    if (flat == 0u) {
        float4 tile;
        [unroll]
        for (uint channel = 0u; channel < RIKKA_TRACED_LIGHTS; ++channel) {
            tile[channel] = tile_low[channel] == tile_high[channel]
                ? float(tile_low[channel]) / float(TILE_LEVELS)
                : RIKKA_TILE_MIXED;
        }
        rikka_storage_rgba16f[push.tiles][group.xy] = tile;
    }
}
