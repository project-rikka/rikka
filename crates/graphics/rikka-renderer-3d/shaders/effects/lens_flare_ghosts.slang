import rikka.effects.lens_flare;

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float CORNER = 0.70710678;

float window(float2 uv) {
    return pow(clamp(1.0 - length(uv - 0.5) / CORNER, 0.0, 1.0), 10.0);
}

float3 dispersed(float2 uv, float2 shift) {
    return float3(
        rikka_effect_sample(push.source, uv + shift).r,
        rikka_effect_sample(push.source, uv).g,
        rikka_effect_sample(push.source, uv - shift).b);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void ghosts(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size) || push.parameters.amount <= 0.0) {
        return;
    }
    Parameters parameters = *push.parameters;
    float2 uv = rikka_effect_uv(int2(id.xy), push.size);
    float aspect = rikka_effect_aspect(push.size);
    float2 flipped = 1.0 - uv;
    float2 toward = (0.5 - flipped) * parameters.ghosts.dispersal;
    float2 across = normalize(float2(toward.x * aspect, toward.y) + float2(1e-6, 0.0));
    float2 shift = across / float2(aspect, 1.0) * parameters.ghosts.chromatic * 0.01;
    float3 sum = float3(0.0);
    for (int ghost = 0; ghost < parameters.ghosts.count; ++ghost) {
        float2 at = flipped + toward * float(ghost);
        if (any(at < 0.0) || any(at > 1.0)) {
            continue;
        }
        float3 tint = lerp(float3(1.0), 2.0 * rikka_glare_spectrum(frac(float(ghost) * 0.37 + 0.1)), 0.5);
        sum += dispersed(at, shift * float(ghost + 1)) * window(at) * tint;
    }
    float2 halo = flipped + across / float2(aspect, 1.0) * parameters.halo.width;
    float ring = pow(clamp(1.0 - length(halo - 0.5) / CORNER, 0.0, 1.0), 5.0);
    if (all(halo >= 0.0) && all(halo <= 1.0)) {
        sum += dispersed(halo, shift * 2.0) * ring * parameters.halo.amount;
    }
    rikka_effect_store(push.target, int2(id.xy), float4(sum, 1.0));
}
