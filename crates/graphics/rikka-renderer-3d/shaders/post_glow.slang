import rikka.post;

struct Glow {
    float3 bloom_tint;
    float bloom_amount;
    float3 streak_tint;
    float streak_amount;
    float3 leak_tint;
    float leak_amount;
    float mist_amount;
    float flare_amount;
    uint additive;
    float sun_facing;
    float2 sun_direction;
    float sun_strength;
    float pad;
};

typealias GlowRef = Glow*;

struct Push {
    GlowRef params;
    uint source;
    uint bloom;
    uint mist;
    uint streak;
    uint flare;
    float time;
    float2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int LEAKS = 3;

float2 perimeter(float around) {
    float side = floor(around * 4.0);
    float along = fract(around * 4.0) * 2.0 - 1.0;
    if (side < 1.0) {
        return float2(along, -1.0);
    }
    if (side < 2.0) {
        return float2(1.0, along);
    }
    if (side < 3.0) {
        return float2(-along, 1.0);
    }
    return float2(-1.0, -along);
}

float3 light_leaks(GlowRef p, float2 uv, float aspect_ratio) {
    float2 aspect = float2(aspect_ratio, 1.0);
    float2 point = (uv - 0.5) * aspect;
    float3 hues[LEAKS] = {
        p.leak_tint,
        p.leak_tint * float3(1.0, 0.72, 0.9),
        p.leak_tint * float3(1.0, 1.1, 0.7) };
    float3 sum = float3(0.0);
    for (int k = 0; k < LEAKS; ++k) {
        float phase = push.time * (0.11 + 0.05 * float(k)) + 2.1 * float(k);
        float drift = (k % 2 == 0 ? 1.0 : -1.0) * push.time * 0.012;
        float around = fract(0.13 + 0.37 * float(k) + 0.05 * sin(phase) + drift);
        float2 anchor = perimeter(around) * aspect * 0.62;
        float radius = 0.45 + 0.15 * sin(phase * 1.3 + float(k));
        float pulse = 0.6 + 0.4 * sin(phase * 0.9 + 1.7 * float(k));
        float2 offset = point - anchor;
        sum += hues[k] * exp(-dot(offset, offset) / (radius * radius)) * pulse;
    }
    if (p.sun_strength > 0.0) {
        float2 anchor = p.sun_direction * aspect * 0.7;
        float2 offset = point - anchor;
        float facing = smoothstep(-0.4, 0.5, p.sun_facing);
        sum += p.leak_tint * float3(1.0, 0.92, 0.8) * exp(-dot(offset, offset) / 0.36)
            * p.sun_strength * facing * 1.5;
    }
    return sum;
}

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    GlowRef p = push.params;
    int2 pixel = int2(frag_coord.xy);
    float2 uv = (float2(pixel) + 0.5) / push.size;
    float4 color = post_fetch(push.source, pixel);
    if (push.bloom != RIKKA_NONE) {
        float4 glow = post_sample(push.bloom, uv);
        color = p.additive != 0u
            ? color + float4(glow.rgb * p.bloom_tint * p.bloom_amount, 0.0)
            : lerp(color, glow * float4(p.bloom_tint, 1.0), p.bloom_amount);
    }
    if (push.mist != RIKKA_NONE) {
        float4 mist = post_sample(push.mist, uv);
        color = p.additive != 0u
            ? color + float4(mist.rgb * (0.4 * p.mist_amount), 0.0)
            : lerp(color, mist, 0.25 * p.mist_amount) + mist * (0.15 * p.mist_amount);
    }
    float3 added = float3(0.0);
    if (push.streak != RIKKA_NONE) {
        added += post_sample(push.streak, uv).rgb * p.streak_tint * p.streak_amount;
    }
    if (push.flare != RIKKA_NONE) {
        added += post_sample(push.flare, uv).rgb * p.flare_amount;
    }
    if (p.leak_amount > 0.0) {
        added += light_leaks(p, uv, push.size.x / push.size.y) * p.leak_amount * 0.5;
    }
    color.rgb += added;
    color.a = clamp(color.a + rikka_luma(added), 0.0, 1.0);
    return color;
}
