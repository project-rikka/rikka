import rikka.common;

static const uint GROUP_SIZE = 256u;
static const uint RIKKA_PYRAMID_LEVELS = 16;
static const uint RIKKA_TILE_LEVELS = 6u;

[[vk::binding(3, 0)]] [[vk::image_format("r32f")]] globallycoherent RWTexture2D<float> rikka_pyramid[];

struct Push {
    RikkaCounterRef counter;
    uint source;
    uint2 source_size;
    uint2 size;
    uint levels;
    uint groups;
    uint destinations[RIKKA_PYRAMID_LEVELS];
};

[[vk::push_constant]] ConstantBuffer<Push> push;

groupshared float reduced[256];
groupshared bool last_group;

uint2 level_size(uint level) {
    return max(push.size >> level, uint2(1u));
}

float farthest_in_footprint(uint2 texel) {
    uint2 low = texel * push.source_size / push.size;
    uint2 high = min(((texel + 1u) * push.source_size + push.size - 1u) / push.size,
                     push.source_size);
    float furthest = 1.0;
    for (uint y = low.y; y < high.y; ++y) {
        for (uint x = low.x; x < high.x; ++x) {
            furthest = min(furthest, rikka_fetch(rikka_images[push.source], int2(int(x), int(y)), 0).r);
        }
    }
    return furthest;
}

void store(uint level, uint2 texel, float value) {
    if (all(texel < level_size(level))) {
        rikka_pyramid[push.destinations[level]][texel] = value;
    }
}

float reduce_image(uint level, uint2 texel) {
    uint2 last = level_size(level - 1u) - 1u;
    uint image = push.destinations[level - 1u];
    float a = rikka_pyramid[image][min(texel * 2u, last)];
    float b = rikka_pyramid[image][min(texel * 2u + uint2(1u, 0u), last)];
    float c = rikka_pyramid[image][min(texel * 2u + uint2(0u, 1u), last)];
    float d = rikka_pyramid[image][min(texel * 2u + uint2(1u), last)];
    return min(min(a, b), min(c, d));
}

[shader("compute")]
[numthreads(256, 1, 1)]
void compute_main(uint3 group_id: SV_GroupID, uint local_index: SV_GroupIndex) {
    uint thread = local_index;
    uint2 local = uint2(thread % 16u, thread / 16u);
    uint2 tile = group_id.xy;

    uint2 last0 = level_size(0u) - 1u;
    uint2 texel1 = tile * 16u + local;
    float value = 1.0;
    [unroll]
    for (uint i = 0u; i < 4u; ++i) {
        uint2 texel0 = texel1 * 2u + uint2(i & 1u, i >> 1u);
        float furthest = farthest_in_footprint(min(texel0, last0));
        store(0u, texel0, furthest);
        value = min(value, furthest);
    }
    if (push.levels > 1u) {
        store(1u, texel1, value);
    }
    reduced[local.y * 16u + local.x] = value;
    GroupMemoryBarrierWithGroupSync();

    uint tile_levels = min(push.levels, RIKKA_TILE_LEVELS);
    for (uint level = 2u; level < tile_levels; ++level) {
        uint across = 16u >> (level - 1u);
        bool reduces = thread < across * across;
        uint2 at = uint2(thread % across, thread / across);
        uint2 base = tile * (32u >> level);
        uint2 last = level_size(level - 1u) - 1u - base * 2u;
        float reduction = 1.0;
        if (reduces) {
            uint2 a = min(at * 2u, last);
            uint2 b = min(at * 2u + uint2(1u, 0u), last);
            uint2 c = min(at * 2u + uint2(0u, 1u), last);
            uint2 d = min(at * 2u + uint2(1u), last);
            reduction = min(
                min(reduced[a.y * 16u + a.x], reduced[b.y * 16u + b.x]),
                min(reduced[c.y * 16u + c.x], reduced[d.y * 16u + d.x]));
            store(level, base + at, reduction);
        }
        GroupMemoryBarrierWithGroupSync();
        if (reduces) {
            reduced[at.y * 16u + at.x] = reduction;
        }
        GroupMemoryBarrierWithGroupSync();
    }

    if (push.levels <= RIKKA_TILE_LEVELS) {
        return;
    }

    DeviceMemoryBarrier();
    GroupMemoryBarrierWithGroupSync();
    if (thread == 0u) {
        uint arrived;
        InterlockedAdd(push.counter[0], 1u, arrived);
        last_group = arrived == push.groups - 1u;
    }
    GroupMemoryBarrierWithGroupSync();
    if (!last_group) {
        return;
    }
    DeviceMemoryBarrier();

    for (uint level = RIKKA_TILE_LEVELS; level < push.levels; ++level) {
        uint2 size = level_size(level);
        for (uint i = thread; i < size.x * size.y; i += GROUP_SIZE) {
            uint2 texel = uint2(i % size.x, i / size.x);
            store(level, texel, reduce_image(level, texel));
        }
        DeviceMemoryBarrier();
        GroupMemoryBarrierWithGroupSync();
    }
}
