import rikka.sky.environment;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint source;
    uint levels;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint SAMPLES = 96u;

float2 hammersley(uint i, uint count) {
    return float2((float(i) + 0.5) / float(count), float(reversebits(i)) * 2.3283064365e-10);
}

float3 base_radiance(float3 direction) {
    float2 uv = rikka_octahedral_uv(direction);
    float size = float(RIKKA_ENVIRONMENT_SIZE);
    uv = clamp(uv, float2(0.5 / size), float2(1.0 - 0.5 / size));
    return rikka_images[push.source].SampleLevel(
        rikka_samplers[RIKKA_ENVIRONMENT_SAMPLER], uv, 0.0).rgb;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    uint2 texel = global_id.xy;
    for (uint level = 0u; level < push.levels; ++level) {
        uint3 rect = rikka_environment_rect(level);
        if (any(texel < rect.xy) || any(texel >= rect.xy + rect.z)) {
            continue;
        }
        float2 uv = (float2(texel - rect.xy) + 0.5) / float(rect.z);
        float3 n = rikka_octahedral_direction(uv);
        float3 result;
        if (level == 0u) {
            result = base_radiance(n);
        } else {
            float roughness = float(level) / float(push.levels - 1u);
            float a = roughness * roughness;
            float3 helper = abs(n.y) < 0.999 ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0);
            float3 tangent = normalize(cross(helper, n));
            float3 bitangent = cross(n, tangent);
            float3 sum = float3(0.0);
            float weight = 0.0;
            for (uint i = 0u; i < SAMPLES; ++i) {
                float2 xi = hammersley(i, SAMPLES);
                float phi = 2.0 * 3.14159265 * xi.x;
                float cos_theta = sqrt((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y));
                float sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
                float3 h = tangent * (cos(phi) * sin_theta) + bitangent * (sin(phi) * sin_theta)
                    + n * cos_theta;
                float3 l = 2.0 * dot(n, h) * h - n;
                float n_dot_l = dot(n, l);
                if (n_dot_l > 0.0) {
                    sum += base_radiance(l) * n_dot_l;
                    weight += n_dot_l;
                }
            }
            result = sum / max(weight, 1e-4);
        }
        rikka_storage_rgba16f[push.target][texel] = float4(result, 1.0);
        return;
    }
}
