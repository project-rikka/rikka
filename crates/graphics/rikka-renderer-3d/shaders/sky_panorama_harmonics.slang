import rikka.sky.panorama;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint source;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint THREADS = 64u;
static const uint LEVEL = 2u;
static const uint DIRECTIONS = 1024u;
static const uint SEARCH_STEPS = 12u;
static const float WIDEST_WINDOW = 0.5;
static const float RINGING = 1e-3;
static const float GOLDEN_ANGLE = 2.39996323;

groupshared float3 gathered[THREADS][9];
groupshared float lowest[THREADS];

void basis(float3 n, out float y[9]) {
    y[0] = 0.282095;
    y[1] = 0.488603 * n.y;
    y[2] = 0.488603 * n.z;
    y[3] = 0.488603 * n.x;
    y[4] = 1.092548 * n.x * n.y;
    y[5] = 1.092548 * n.y * n.z;
    y[6] = 0.315392 * (3.0 * n.z * n.z - 1.0);
    y[7] = 1.092548 * n.x * n.z;
    y[8] = 0.546274 * (n.x * n.x - n.y * n.y);
}

float band_window(uint band, float narrowing) {
    float x = RIKKA_PI * float(band) * narrowing;
    if (band == 0u || x <= 0.0) {
        return 1.0;
    }
    if (x >= RIKKA_PI) {
        return 0.0;
    }
    float sinc = sin(x) / x;
    return sinc * sinc * sinc * sinc;
}

float3 irradiance(float3 c[9], float3 n, float narrowing) {
    float y[9];
    basis(n, y);
    float first = band_window(1u, narrowing);
    float second = band_window(2u, narrowing);
    float3 result = c[0] * y[0];
    [unroll]
    for (uint k = 1u; k < 4u; ++k) {
        result += c[k] * (y[k] * first);
    }
    [unroll]
    for (uint k = 4u; k < 9u; ++k) {
        result += c[k] * (y[k] * second);
    }
    return result;
}

float3 spread_direction(uint index) {
    float up = 1.0 - (float(index) + 0.5) * (2.0 / float(DIRECTIONS));
    float around = sqrt(max(1.0 - up * up, 0.0));
    float azimuth = GOLDEN_ANGLE * float(index);
    return float3(cos(azimuth) * around, up, sin(azimuth) * around);
}

float ringing(float3 c[9], uint index, float narrowing) {
    float3 mean = max(c[0] * 0.282095, float3(1e-12));
    float worst = 1.0;
    for (uint d = index; d < DIRECTIONS; d += THREADS) {
        float3 ratio = irradiance(c, spread_direction(d), narrowing) / mean;
        worst = min(worst, min(ratio.x, min(ratio.y, ratio.z)));
    }
    lowest[index] = worst;
    GroupMemoryBarrierWithGroupSync();
    for (uint stride = THREADS / 2u; stride > 0u; stride >>= 1u) {
        if (index < stride) {
            lowest[index] = min(lowest[index], lowest[index + stride]);
        }
        GroupMemoryBarrierWithGroupSync();
    }
    float found = lowest[0];
    GroupMemoryBarrierWithGroupSync();
    return found;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void compute_main(uint local_index: SV_GroupIndex) {
    uint index = local_index;
    RikkaFrameRef frame = push.frame;
    RikkaSky sky = frame.look.sky;
    Texture2D atlas = rikka_images[NonUniformResourceIndex(push.source)];
    uint4 rect = rikka_panorama_level(LEVEL);
    float across = RIKKA_TAU / float(rect.z);

    float3 sums[9];
    [unroll]
    for (uint k = 0u; k < 9u; ++k) {
        sums[k] = float3(0.0);
    }
    for (uint texel = index; texel < rect.z * rect.w; texel += THREADS) {
        uint2 at = uint2(texel % rect.z, texel / rect.z);
        float top = float(at.y) / float(rect.w) * RIKKA_PI;
        float bottom = float(at.y + 1u) / float(rect.w) * RIKKA_PI;
        float solid_angle = across * (cos(top) - cos(bottom));
        float2 uv = (float2(at) + 0.5) / float2(rect.zw);
        float3 n = rikka_panorama_to_world(sky, rikka_panorama_direction(uv));
        float3 radiance = rikka_fetch(atlas, int2(rect.xy + at), 0).rgb * solid_angle;
        float y[9];
        basis(n, y);
        [unroll]
        for (uint k = 0u; k < 9u; ++k) {
            sums[k] += radiance * y[k];
        }
    }
    [unroll]
    for (uint k = 0u; k < 9u; ++k) {
        gathered[index][k] = sums[k];
    }
    GroupMemoryBarrierWithGroupSync();
    for (uint stride = THREADS / 2u; stride > 0u; stride >>= 1u) {
        if (index < stride) {
            [unroll]
            for (uint k = 0u; k < 9u; ++k) {
                gathered[index][k] += gathered[index + stride][k];
            }
        }
        GroupMemoryBarrierWithGroupSync();
    }

    float3 c[9];
    c[0] = gathered[0][0];
    [unroll]
    for (uint k = 1u; k < 4u; ++k) {
        c[k] = gathered[0][k] * (2.0 / 3.0);
    }
    [unroll]
    for (uint k = 4u; k < 9u; ++k) {
        c[k] = gathered[0][k] * 0.25;
    }

    float narrowing = 0.0;
    if (ringing(c, index, 0.0) < -RINGING) {
        float low = 0.0;
        float high = WIDEST_WINDOW;
        for (uint step = 0u; step < SEARCH_STEPS; ++step) {
            float middle = 0.5 * (low + high);
            if (ringing(c, index, middle) < -RINGING) {
                low = middle;
            } else {
                high = middle;
            }
        }
        narrowing = high;
    }

    if (index == 0u) {
        RikkaEnvironmentRef environment = frame.environment;
        float first = band_window(1u, narrowing);
        float second = band_window(2u, narrowing);
        environment.irradiance[0] = float4(c[0], 0.0);
        [unroll]
        for (uint k = 1u; k < 4u; ++k) {
            environment.irradiance[k] = float4(c[k] * first, 0.0);
        }
        [unroll]
        for (uint k = 4u; k < 9u; ++k) {
            environment.irradiance[k] = float4(c[k] * second, 0.0);
        }
    }
}
