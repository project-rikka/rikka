import rikka.space.view;

struct Push {
    RikkaFrameRef frame;
    uint source;
    uint target;
    uint2 size;
    uint2 half_size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float SPREAD = 1.6;
static const float DEPTH_TOLERANCE = 0.02;
static const float NORMAL_POWER = 8.0;
static const int TILE = 8;
static const int HALF_TILE = TILE / 2;
static const int REACH = 2;

groupshared float4 tile_ambient[TILE * TILE];
groupshared float4 tile_surface[TILE * TILE];

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID, uint3 group_id: SV_GroupID, uint3 local_id: SV_GroupThreadID) {
    RikkaScene scene = push.frame.look.scene;
    int2 highest = int2(push.half_size) - 1;
    int2 full_highest = int2(push.size) - 1;
    int2 corner = int2(group_id.xy) * HALF_TILE - REACH;
    int2 loaded = clamp(corner + int2(local_id.xy), int2(0), highest);
    float4 raw = rikka_fetch(rikka_images[push.source], loaded, 0);
    uint slot = local_id.x + local_id.y * uint(TILE);
    tile_ambient[slot] = float4(raw.r, raw.r * space_decode_direction(raw.gb));
    tile_surface[slot] = float4(rikka_read_surface(scene.surface, min(loaded * 2, full_highest)).normal, raw.a);
    GroupMemoryBarrierWithGroupSync();

    int2 pixel = int2(global_id.xy);
    if (any(uint2(pixel) >= push.size)) {
        return;
    }
    SpaceView v = space_view(push.frame);
    float depth = rikka_read_depth(scene.depth, pixel);
    if (depth <= 0.0) {
        rikka_storage_rgba16f[push.target][uint2(pixel)] = float4(1.0, 0.0, 0.0, 0.0);
        return;
    }
    float distance = space_distance(v, depth);
    float3 normal = rikka_read_surface(scene.surface, pixel).normal;
    float tolerance = DEPTH_TOLERANCE * distance + 0.01;

    float2 centre = (float2(pixel) + 0.5) * 0.5 - 0.5;
    int2 base = int2(floor(centre)) - 1;
    float4 sum = float4(0.0);
    float total = 0.0;
    float4 nearest = float4(1.0, normal);
    float nearest_gap = 1e30;
    [unroll]
    for (int y = 0; y < 4; ++y) {
        [unroll]
        for (int x = 0; x < 4; ++x) {
            int2 at = base + int2(x, y);
            int2 local = at - corner;
            uint index = uint(local.x + local.y * TILE);
            float4 surface = tile_surface[index];
            if (surface.w <= 0.0) {
                continue;
            }
            float4 ambient = tile_ambient[index];
            float gap = abs(surface.w - distance);
            if (gap < nearest_gap) {
                nearest_gap = gap;
                nearest = ambient;
            }
            float2 offset = float2(clamp(at, int2(0), highest)) - centre;
            float spatial = exp(-dot(offset, offset) / (2.0 * SPREAD * SPREAD));
            float along = exp(-gap / tolerance);
            float facing = pow(max(dot(normal, surface.xyz), 0.0), NORMAL_POWER);
            float weight = spatial * along * facing;
            sum += ambient * weight;
            total += weight;
        }
    }
    float4 result = total > 1e-4 ? sum / total : nearest;
    rikka_storage_rgba16f[push.target][uint2(pixel)] = float4(result.x, result.yzw - result.x * normal);
}
