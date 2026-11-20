import rikka.common;

struct Push {
    uint target;
    uint size;
    uint samples;
    uint _pad;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

float2 hammersley(uint index, uint count) {
    uint bits = reversebits(index);
    return float2((float(index) + 0.5) / float(count), float(bits) * 2.3283064365386963e-10);
}

float smith_lambda(float cosine, float a2) {
    float c2 = max(cosine * cosine, 1e-7);
    return 0.5 * (-1.0 + sqrt(1.0 + a2 * (1.0 - c2) / c2));
}

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    if (any(global_id.xy >= uint2(push.size))) {
        return;
    }
    float last = float(push.size - 1u);
    float mu = max(float(global_id.x) / last, 1e-3);
    float roughness = float(global_id.y) / last;
    float alpha = max(roughness * roughness, 2e-3);
    float a2 = alpha * alpha;
    float3 v = float3(sqrt(1.0 - mu * mu), 0.0, mu);
    float scale = 0.0;
    float bias = 0.0;
    for (uint i = 0u; i < push.samples; ++i) {
        float2 xi = hammersley(i, push.samples);
        float phi = 2.0 * RIKKA_PI * xi.y;
        float cos_theta = min(sqrt((1.0 - xi.x) / (1.0 + (a2 - 1.0) * xi.x)), 1.0);
        float sin_theta = sqrt(max(1.0 - cos_theta * cos_theta, 0.0));
        float3 h = float3(sin_theta * cos(phi), sin_theta * sin(phi), cos_theta);
        float v_dot_h = dot(v, h);
        float3 l = 2.0 * v_dot_h * h - v;
        if (l.z <= 0.0 || v_dot_h <= 0.0) {
            continue;
        }
        float g2 = 1.0 / (1.0 + smith_lambda(v.z, a2) + smith_lambda(l.z, a2));
        float weight = g2 * v_dot_h / (h.z * v.z);
        float fc = pow(1.0 - v_dot_h, 5.0);
        scale += (1.0 - fc) * weight;
        bias += fc * weight;
    }
    float count = float(push.samples);
    rikka_storage_rgba16f[push.target][global_id.xy] = float4(scale / count, bias / count, 0.0, 1.0);
}
