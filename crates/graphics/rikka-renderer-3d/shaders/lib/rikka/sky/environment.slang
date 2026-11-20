__exported import rikka.common;

static const uint RIKKA_ENVIRONMENT_SIZE = 128u;
static const uint RIKKA_ENVIRONMENT_SAMPLER = 1u;

float2 rikka_sign_not_zero(float2 v) {
    return float2(v.x >= 0.0 ? 1.0 : -1.0, v.y >= 0.0 ? 1.0 : -1.0);
}

float2 rikka_octahedral_uv(float3 direction) {
    float3 n = direction / max(abs(direction.x) + abs(direction.y) + abs(direction.z), 1e-8);
    float2 e = n.xz;
    if (n.y < 0.0) {
        e = (1.0 - abs(e.yx)) * rikka_sign_not_zero(e);
    }
    return e * 0.5 + 0.5;
}

float3 rikka_octahedral_direction(float2 uv) {
    float2 e = uv * 2.0 - 1.0;
    float3 n = float3(e.x, 1.0 - abs(e.x) - abs(e.y), e.y);
    if (n.y < 0.0) {
        n.xz = (1.0 - abs(n.zx)) * rikka_sign_not_zero(n.xz);
    }
    return normalize(n);
}

uint3 rikka_environment_rect(uint level) {
    uint size = RIKKA_ENVIRONMENT_SIZE >> level;
    if (level == 0u) {
        return uint3(0u, 0u, size);
    }
    return uint3(RIKKA_ENVIRONMENT_SIZE, RIKKA_ENVIRONMENT_SIZE - 2u * size, size);
}

float3 rikka_environment_texel(uint image, float3 direction, uint level) {
    uint3 rect = rikka_environment_rect(level);
    float size = float(rect.z);
    float2 texel = float2(rect.xy) + clamp(rikka_octahedral_uv(direction) * size,
                                           float2(0.5), float2(size - 0.5));
    float2 atlas = float2(rikka_image_size(rikka_images[image], 0));
    return rikka_images[image].SampleLevel(rikka_samplers[RIKKA_ENVIRONMENT_SAMPLER],
                                           texel / atlas, 0.0).rgb;
}
