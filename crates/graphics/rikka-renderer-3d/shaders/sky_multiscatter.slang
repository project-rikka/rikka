import rikka.sky.atmosphere;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int STEPS = 20;
static const uint DIRECTIONS = 64u;

groupshared float3 gathered_light[DIRECTIONS];
groupshared float3 gathered_fraction[DIRECTIONS];

[shader("compute")]
[numthreads(64, 1, 1)]
void compute_main(uint3 group_id: SV_GroupID, uint local_index: SV_GroupIndex) {
    int2 size = int2(rikka_image_size(rikka_storage_rgba16f[push.target]));
    int2 texel = int2(group_id.xy);
    RikkaSky sky = push.frame.look.sky;
    float2 uv = (float2(texel) + 0.5) / float2(size);
    float sun_mu = clamp(rikka_from_sub_uv(uv.x, float(size.x)), 0.0, 1.0) * 2.0 - 1.0;
    float altitude = clamp(rikka_from_sub_uv(uv.y, float(size.y)), 0.0, 1.0)
        * (RIKKA_ATMOSPHERE_RADIUS - RIKKA_PLANET_RADIUS);
    float3 origin = float3(0.0, RIKKA_PLANET_RADIUS + max(altitude, 0.01), 0.0);
    float3 to_sun = float3(0.0, sun_mu, sqrt(max(1.0 - sun_mu * sun_mu, 0.0)));

    uint index = local_index;
    float azimuth = (float(index % 8u) + 0.5) / 8.0 * 2.0 * RIKKA_PI;
    float cos_polar = 1.0 - 2.0 * (float(index / 8u) + 0.5) / 8.0;
    float sin_polar = sqrt(max(1.0 - cos_polar * cos_polar, 0.0));
    float3 direction = float3(cos(azimuth) * sin_polar, cos_polar, sin(azimuth) * sin_polar);

    float span = rikka_atmosphere_path(origin, direction);
    float step = span / float(STEPS);
    float3 light = float3(0.0);
    float3 fraction = float3(0.0);
    float3 throughput = float3(1.0);
    for (int i = 0; i < STEPS; ++i) {
        float3 at = origin + direction * ((float(i) + 0.5) * step);
        RikkaMedium medium = rikka_medium(length(at) - RIKKA_PLANET_RADIUS, sky.haze);
        float3 passed = exp(-medium.extinction * step);
        float3 lit = rikka_transmittance_to(sky, at, to_sun) * medium.scattering
            / (4.0 * RIKKA_PI);
        float3 extinction = max(medium.extinction, float3(1e-7));
        light += throughput * (lit - lit * passed) / extinction;
        fraction += throughput * (medium.scattering - medium.scattering * passed) / extinction;
        throughput *= passed;
    }
    if (rikka_hits_ground(origin, direction)) {
        float3 ground = origin + direction * span;
        float facing = clamp(dot(normalize(ground), to_sun), 0.0, 1.0);
        light += throughput * rikka_transmittance_to(sky, ground * 1.00001, to_sun) * facing
            * sky.ground / RIKKA_PI;
    }
    gathered_light[index] = light;
    gathered_fraction[index] = fraction;
    GroupMemoryBarrierWithGroupSync();
    for (uint stride = DIRECTIONS / 2u; stride > 0u; stride >>= 1u) {
        if (index < stride) {
            gathered_light[index] += gathered_light[index + stride];
            gathered_fraction[index] += gathered_fraction[index + stride];
        }
        GroupMemoryBarrierWithGroupSync();
    }
    if (index == 0u) {
        float3 second = gathered_light[0] / float(DIRECTIONS);
        float3 transfer = gathered_fraction[0] / float(DIRECTIONS);
        float3 psi = second / max(float3(1.0) - transfer, float3(1e-4));
        rikka_storage_rgba16f[push.target][uint2(texel)] = float4(psi, 1.0);
    }
}
