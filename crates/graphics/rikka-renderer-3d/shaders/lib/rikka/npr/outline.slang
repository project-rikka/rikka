__exported import rikka.npr.rim;

float rikka_npr_piecewise(float z, float near, float middle, float far, float near_scale, float middle_scale, float far_scale) {
    if (z <= near) {
        return near_scale;
    }
    if (z < middle) {
        return lerp(near_scale, middle_scale, rikka_npr_linearstep(near, middle, z));
    }
    if (z < far) {
        return lerp(middle_scale, far_scale, rikka_npr_linearstep(middle, far, z));
    }
    return far_scale;
}

float rikka_npr_eye_distance(RikkaContext context, float depth, float scale, float fov) {
    float z = depth / max(scale, 1e-4);
    return fov > 0.5 ? z * RIKKA_NPR_REFERENCE_FOCAL / rikka_npr_focal(context) : z;
}

float rikka_npr_outline_width(
    RikkaContext context, float3 position, float4 color, float width, float mode, float lowest, float highest, float fov,
    float scale, float near, float middle, float far, float near_scale, float middle_scale, float far_scale,
    float channel, float multiplier)
{
    float depth = max(rikka_npr_depth(context, position), context.view.near);
    float projected = context.view.projection[3][3] < 0.5 ? depth : 1.0;
    float to_pixels = 0.5 * context.view.viewport.y * rikka_npr_focal(context) / projected;
    float w;
    switch (int(mode)) {
    case 1:
        w = width / to_pixels;
        break;
    case 2:
        w = width * scale * rikka_npr_piecewise(
            rikka_npr_eye_distance(context, depth, scale, fov), near, middle, far, near_scale, middle_scale, far_scale);
        break;
    case 3:
        w = width * projected / rikka_npr_focal(context);
        break;
    default:
        w = width * scale;
        break;
    }
    w *= rikka_npr_channel(color, channel, 1.0) * multiplier;
    if (w > 0.0 && (lowest > 0.0 || highest > 0.0)) {
        float upper = highest > 0.0 ? highest : 1e9;
        w = clamp(w * to_pixels, min(lowest, upper), upper) / to_pixels;
    }
    return w;
}

float3 rikka_npr_outline_direction(
    RikkaContext context, float3 normal, float3 smoothed, float3 tangent, float4 color, float3 custom, float source,
    float flatten, float flatten_z)
{
    float3 direction;
    switch (int(source)) {
    case 1:
        direction = normal;
        break;
    case 2:
        direction = tangent;
        break;
    case 3: {
        float3 n = normalize(normal);
        float3 t = tangent - n * dot(n, tangent);
        t = dot(t, t) > 1e-10 ? normalize(t) : (abs(n.y) < 0.999 ? normalize(cross(float3(0.0, 1.0, 0.0), n)) : float3(1.0, 0.0, 0.0));
        float3 b = cross(n, t);
        float3 local = color.rgb * 2.0 - 1.0;
        direction = t * local.x + b * local.y + n * local.z;
        break;
    }
    case 4:
        direction = custom;
        break;
    default:
        direction = smoothed;
        break;
    }
    direction = dot(direction, direction) > 1e-10 ? normalize(direction) : normal;
    if (flatten > 0.5) {
        float3 v = rikka_npr_to_view(context, direction);
        v.z = flatten_z;
        v.xy = normalize(v).xy;
        direction = rikka_npr_from_view(context, v);
    }
    return direction;
}

float rikka_npr_outline_depth(RikkaContext context, float3 position, float offset, float mode, float scale, float fov, float mask) {
    float depth = max(rikka_npr_depth(context, position), context.view.near);
    float pushed = offset * scale;
    if (int(mode) == 1) {
        float z = rikka_npr_eye_distance(context, depth, scale, fov);
        pushed = depth * lerp(0.1 * offset, offset, saturate(1.0 - z)) * scale;
    }
    return pushed * mask;
}

float3 rikka_npr_flatten(RikkaContext context, float3 position, float amount, float pivot_mode, float3 pivot) {
    float3 center;
    switch (int(pivot_mode)) {
    case 1:
        center = context.instance.center;
        break;
    case 2:
        center = pivot;
        break;
    default:
        center = rikka_npr_head(context).center;
        break;
    }
    float3 here = mul(context.view.view, float4(position, 1.0)).xyz;
    float3 around = mul(context.view.view, float4(center, 1.0)).xyz;
    if (abs(around.z) < 1e-4) {
        return float3(0.0);
    }
    float3 moved = here;
    moved.xy *= lerp(1.0, here.z / around.z, amount);
    return rikka_npr_from_view(context, moved - here);
}
