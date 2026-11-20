import rikka.sky.dome;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint THREADS = 64u;
static const uint AZIMUTHS = 32u;
static const uint RINGS = 16u;
static const uint TERMS = 10u;

groupshared float3 gathered[THREADS][TERMS];

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

[shader("compute")]
[numthreads(64, 1, 1)]
void compute_main(uint local_index: SV_GroupIndex) {
    uint index = local_index;
    RikkaFrameRef frame = push.frame;
    RikkaSky sky = frame.look.sky;
    float time = frame.view.time;
    float3 ambient = rikka_clear_sky_ambient(sky);
    float solid_angle = 2.0 * RIKKA_PI / float(AZIMUTHS * RINGS);

    float3 sums[TERMS];
    [unroll]
    for (uint k = 0u; k < TERMS; ++k) {
        sums[k] = float3(0.0);
    }
    for (uint sample_index = index; sample_index < AZIMUTHS * RINGS; sample_index += THREADS) {
        float azimuth = (float(sample_index % AZIMUTHS) + 0.5) / float(AZIMUTHS)
            * 2.0 * RIKKA_PI;
        float up = (float(sample_index / AZIMUTHS) + 0.5) / float(RINGS);
        float around = sqrt(max(1.0 - up * up, 0.0));
        float3 n = float3(cos(azimuth) * around, up, sin(azimuth) * around);
        float3 radiance = rikka_sky_dome(sky, n, time, ambient, float3(0.0)).radiance * solid_angle;
        float y[9];
        basis(n, y);
        [unroll]
        for (uint k = 0u; k < 9u; ++k) {
            sums[k] += radiance * y[k];
        }
        sums[9] += radiance * up / RIKKA_PI;
    }
    [unroll]
    for (uint k = 0u; k < TERMS; ++k) {
        gathered[index][k] = sums[k];
    }
    GroupMemoryBarrierWithGroupSync();
    for (uint stride = THREADS / 2u; stride > 0u; stride >>= 1u) {
        if (index < stride) {
            [unroll]
            for (uint k = 0u; k < TERMS; ++k) {
                gathered[index][k] += gathered[index + stride][k];
            }
        }
        GroupMemoryBarrierWithGroupSync();
    }
    if (index == 0u) {
        float3 ground = rikka_ground_radiance(sky, gathered[0][9]);
        float3 c[9];
        [unroll]
        for (uint k = 0u; k < 9u; ++k) {
            c[k] = gathered[0][k];
        }
        c[0] += ground * (0.282095 * 2.0 * RIKKA_PI);
        c[1] -= ground * (0.488603 * RIKKA_PI);
        RikkaEnvironmentRef environment = frame.environment;
        environment.irradiance[0] = float4(c[0], 0.0);
        [unroll]
        for (uint k = 1u; k < 4u; ++k) {
            environment.irradiance[k] = float4(c[k] * (2.0 / 3.0), 0.0);
        }
        [unroll]
        for (uint k = 4u; k < 9u; ++k) {
            environment.irradiance[k] = float4(c[k] * 0.25, 0.0);
        }
    }
}
