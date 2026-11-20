import rikka.sky.panorama;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint source;
    uint levels;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint SAMPLES = 128u;
static const float FILTER_BIAS = 1.0;

float2 hammersley(uint i, uint count) {
    return float2((float(i) + 0.5) / float(count), float(reversebits(i)) * 2.3283064365e-10);
}

float lod_for(float sample_solid_angle, float3 direction) {
    float polar = acos(clamp(direction.y, -1.0, 1.0));
    return 0.5 * log2(sample_solid_angle / rikka_panorama_texel_solid_angle(polar));
}

float3 radiance(RikkaSky sky, float3 world, float sample_solid_angle, float bias) {
    float3 direction = rikka_panorama_from_world(sky, world);
    return rikka_panorama_sample(push.source, direction, lod_for(sample_solid_angle, direction) + bias);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 texel = global_id.xy;
    RikkaSky sky = push.frame.look.sky;
    for (uint level = 0u; level < push.levels; ++level) {
        uint3 rect = rikka_environment_rect(level);
        if (any(texel < rect.xy) || any(texel >= rect.xy + rect.z)) {
            continue;
        }
        float2 uv = (float2(texel - rect.xy) + 0.5) / float(rect.z);
        float3 n = rikka_octahedral_direction(uv);
        float3 result;
        if (level == 0u) {
            float size = float(RIKKA_ENVIRONMENT_SIZE);
            result = radiance(sky, n, 4.0 * RIKKA_PI / (size * size), 0.0);
        } else {
            float roughness = float(level) / float(push.levels - 1u);
            float a = roughness * roughness;
            float a2 = a * a;
            float3 helper = abs(n.y) < 0.999 ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0);
            float3 tangent = normalize(cross(helper, n));
            float3 bitangent = cross(n, tangent);
            float3 sum = float3(0.0);
            float weight = 0.0;
            for (uint i = 0u; i < SAMPLES; ++i) {
                float2 xi = hammersley(i, SAMPLES);
                float phi = RIKKA_TAU * xi.x;
                float cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a2 - 1.0) * xi.y));
                float sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
                float3 h = tangent * (cos(phi) * sin_theta) + bitangent * (sin(phi) * sin_theta)
                    + n * cos_theta;
                float3 l = 2.0 * dot(n, h) * h - n;
                float n_dot_l = dot(n, l);
                if (n_dot_l <= 0.0) {
                    continue;
                }
                float lobe = (cos_theta * cos_theta) * (a2 - 1.0) + 1.0;
                float density = a2 / (RIKKA_PI * lobe * lobe) * 0.25;
                float sample_solid_angle = 1.0 / (float(SAMPLES) * max(density, 1e-6));
                sum += radiance(sky, l, sample_solid_angle, FILTER_BIAS) * n_dot_l;
                weight += n_dot_l;
            }
            result = sum / max(weight, 1e-4);
        }
        rikka_storage_rgba16f[NonUniformResourceIndex(push.target)][texel] = float4(result, 1.0);
        return;
    }
}
