__exported import rikka.common;

float4 rikka_unjittered(RikkaViewRef view, float4 clip) {
    return float4(clip.xy - view.jitter * clip.w, clip.zw);
}

float2 rikka_motion(float4 clip, float4 previous_clip) {
    float2 current = clip.xy / clip.w;
    float2 previous = previous_clip.w > 0.0 ? previous_clip.xy / previous_clip.w : current;
    return (current - previous) * 0.5;
}

float4 rikka_unjittered_point(RikkaViewRef view, float2 uv, float depth) {
    return mul(view.inverse_view_projection, float4(uv * 2.0 - 1.0 + view.jitter, depth, 1.0));
}

float2 rikka_sky_motion(RikkaViewRef view, float4x4 previous_view_projection, float2 uv) {
    float4 far = rikka_unjittered_point(view, uv, 0.0);
    float4 before = mul(previous_view_projection, float4(far.xyz, 0.0));
    if (abs(before.w) <= 1e-7) {
        return float2(0.0);
    }
    float2 previous = before.xy / before.w * 0.5 + 0.5;
    return uv - previous;
}

float2 rikka_point_motion(RikkaViewRef view, float4x4 previous_view_projection, float2 uv, float depth) {
    if (depth <= 0.0) {
        return rikka_sky_motion(view, previous_view_projection, uv);
    }
    float4 point = rikka_unjittered_point(view, uv, depth);
    float4 before = mul(previous_view_projection, float4(point.xyz / point.w, 1.0));
    if (before.w <= 1e-7) {
        return float2(0.0);
    }
    return uv - (before.xy / before.w * 0.5 + 0.5);
}

float2 rikka_history_uv(RikkaViewRef view, float2 uv, float2 velocity) {
    return uv - velocity + (view.previous_jitter - view.jitter) * 0.5;
}

float2 rikka_previous_image_uv(RikkaViewRef view, float2 uv) {
    return uv + view.previous_jitter * 0.5;
}
