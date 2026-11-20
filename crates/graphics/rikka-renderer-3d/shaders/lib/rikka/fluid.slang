__exported import rikka.common;

static const float2 RIKKA_FLUID_EDGE = float2(0.15, 0.5);
static const float RIKKA_FLUID_SPREAD = 1.6;
static const float RIKKA_FLUID_OUTLINE = 0.6;
static const float RIKKA_FLUID_PROBE = 1.5;
static const float2 RIKKA_FLUID_BROAD = float2(0.6, 0.9);
static const uint RIKKA_FLUID_LINEAR_CLAMP = 1u;
static const float RIKKA_FLUID_SPARSEST = 0.05;
static const float RIKKA_FLUID_DEPTH_BIAS = 1.5;
static const uint RIKKA_FLUID_THICKNESS_DIVISOR = 2u;
static const int RIKKA_FLUID_FILL_REACH = 24;
static const float RIKKA_FLUID_DIFFUSION = 2.0;
static const int RIKKA_FLUID_DIFFUSION_REACH = 24;
static const float RIKKA_FLUID_STEEPEST = 3.0;
static const float RIKKA_FLUID_CLEAR = 0.01;
static const int2 RIKKA_FLUID_WAYS[8] = {
    int2(1, 0), int2(0, 1), int2(1, 1), int2(1, -1),
    int2(-1, 0), int2(0, -1), int2(-1, -1), int2(-1, 1) };

struct RikkaFluidBounds {
    float2 low;
    float2 high;
    float depth;
};

float3x3 rikka_fluid_rotation(float4 q) {
    q = dot(q, q) > 1e-12 ? normalize(q) : float4(0.0, 0.0, 0.0, 1.0);
    float xx = q.x * q.x;
    float yy = q.y * q.y;
    float zz = q.z * q.z;
    float xy = q.x * q.y;
    float xz = q.x * q.z;
    float yz = q.y * q.z;
    float wx = q.w * q.x;
    float wy = q.w * q.y;
    float wz = q.w * q.z;
    return rikka_columns(
        float3(1.0 - 2.0 * (yy + zz), 2.0 * (xy + wz), 2.0 * (xz - wy)),
        float3(2.0 * (xy - wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz + wx)),
        float3(2.0 * (xz + wy), 2.0 * (yz - wx), 1.0 - 2.0 * (xx + yy)));
}

bool rikka_fluid_extent(float4 row, float4 w_row, out float2 range) {
    range = float2(0.0);
    float a = dot(w_row.xyz, w_row.xyz) - w_row.w * w_row.w;
    float b = dot(row.xyz, w_row.xyz) - row.w * w_row.w;
    float c = dot(row.xyz, row.xyz) - row.w * row.w;
    float discriminant = b * b - a * c;
    if (discriminant < 0.0 || abs(a) < 1e-30) {
        return false;
    }
    float root = sqrt(discriminant);
    float2 ends = float2(b - root, b + root) / a;
    range = float2(min(ends.x, ends.y), max(ends.x, ends.y));
    return true;
}

bool rikka_fluid_bounds(float4x4 view_projection, float3 center, float3x3 axes, float3 radii,
                        out RikkaFluidBounds bounds) {
    bounds = {};
    float4x4 shape = rikka_columns(
        float4(rikka_column(axes, 0) * radii.x, 0.0),
        float4(rikka_column(axes, 1) * radii.y, 0.0),
        float4(rikka_column(axes, 2) * radii.z, 0.0),
        float4(center, 1.0));
    float4x4 m = mul(view_projection, shape);
    float4 row_x = m[0];
    float4 row_y = m[1];
    float4 row_z = m[2];
    float4 row_w = m[3];
    float reach = length(row_w.xyz);
    if (row_w.w - reach <= 1e-5) {
        if (row_w.w + reach <= 1e-5) {
            return false;
        }
        bounds.low = float2(-1.0);
        bounds.high = float2(1.0);
        bounds.depth = 1.0;
        return true;
    }
    float2 across;
    float2 down;
    float2 deep;
    if (!rikka_fluid_extent(row_x, row_w, across)
        || !rikka_fluid_extent(row_y, row_w, down)
        || !rikka_fluid_extent(row_z, row_w, deep)) {
        return false;
    }
    bounds.low = float2(across.x, down.x);
    bounds.high = float2(across.y, down.y);
    bounds.depth = clamp(deep.y, 0.0, 1.0);
    return true;
}

float3 rikka_fluid_unproject(float4x4 inverse_view_projection, float2 ndc, float depth, out float w) {
    float4 h = mul(inverse_view_projection, float4(ndc, depth, 1.0));
    w = 1.0 / h.w;
    return h.xyz * w;
}

bool rikka_fluid_hit(float3 origin, float3 direction, out float2 span) {
    span = float2(0.0);
    float a = dot(direction, direction);
    float b = dot(origin, direction);
    float c = dot(origin, origin) - 1.0;
    float discriminant = b * b - a * c;
    if (discriminant < 0.0 || a <= 0.0) {
        return false;
    }
    float root = sqrt(discriminant);
    span = float2(-b - root, -b + root) / a;
    return span.y > 0.0;
}

float rikka_fluid_depth_at(float4 clip, float t) {
    return (clip.x + t * (clip.z - clip.x)) / (clip.y + t * (clip.w - clip.y));
}

float rikka_fluid_distance_of(float4x4 p, float depth) {
    float denominator = depth * p[3][2] - p[2][2];
    if (abs(denominator) < 1e-12) {
        return 1e30;
    }
    return -(p[2][3] - depth * p[3][3]) / denominator;
}

float rikka_fluid_depth_of(float4x4 p, float distance) {
    return (p[2][3] - distance * p[2][2]) / (p[3][3] - distance * p[3][2]);
}

float rikka_fluid_biased_depth(float4x4 projection, float radius, float depth) {
    if (depth <= 0.0) {
        return depth;
    }
    float distance = rikka_fluid_distance_of(projection, depth);
    return rikka_fluid_depth_of(projection, distance + RIKKA_FLUID_DEPTH_BIAS * radius);
}

float rikka_fluid_pixels_per_metre(RikkaViewRef view, float distance) {
    float4x4 p = view.projection;
    float w = p[3][3] - distance * p[3][2];
    return 0.5 * view.viewport.y * abs(p[1][1]) / max(w, 1e-6);
}

float3 rikka_fluid_position(RikkaViewRef view, float2 uv, float distance) {
    float depth = rikka_fluid_depth_of(view.projection, distance);
    float4 world = mul(view.inverse_view_projection, float4(uv * 2.0 - 1.0, depth, 1.0));
    return world.xyz / world.w;
}

float3 rikka_fluid_eye(RikkaViewRef view, float3 position) {
    return view.projection[3][2] == 0.0
        ? -view.camera_forward
        : normalize(view.camera_position - position);
}

float3 rikka_fluid_tangent(RikkaViewRef view, int2 pixel, int2 toward, float3 position,
                           float distance, float ahead_distance, float behind_distance) {
    int2 limit = int2(view.viewport) - 1;
    int2 ahead = clamp(pixel + toward, int2(0), limit);
    int2 behind = clamp(pixel - toward, int2(0), limit);
    bool has_ahead = ahead_distance > 0.0 && any(ahead != pixel);
    bool has_behind = behind_distance > 0.0 && any(behind != pixel);
    bool use_ahead = has_ahead
        && (!has_behind || abs(ahead_distance - distance) <= abs(distance - behind_distance));
    if (use_ahead) {
        float2 uv = (float2(ahead) + 0.5) * view.inverse_viewport;
        return rikka_fluid_position(view, uv, ahead_distance) - position;
    }
    if (has_behind) {
        float2 uv = (float2(behind) + 0.5) * view.inverse_viewport;
        return position - rikka_fluid_position(view, uv, behind_distance);
    }
    return float3(0.0);
}

float3 rikka_fluid_step(RikkaViewRef view, uint smoothed, int2 pixel, int2 toward,
                        float3 position, float distance) {
    int2 limit = int2(view.viewport) - 1;
    float ahead = rikka_fetch(rikka_images[smoothed], clamp(pixel + toward, int2(0), limit), 0).r;
    float behind = rikka_fetch(rikka_images[smoothed], clamp(pixel - toward, int2(0), limit), 0).r;
    return rikka_fluid_tangent(view, pixel, toward, position, distance, ahead, behind);
}

float3 rikka_fluid_facing(float3 across, float3 down, float3 eye) {
    float3 normal = cross(across, down);
    float size = length(normal);
    if (!(size > 1e-12)) {
        return eye;
    }
    normal /= size;
    return dot(normal, eye) < 0.0 ? -normal : normal;
}

float rikka_fluid_spread(RikkaViewRef view, RikkaFluidRef fluid, float distance) {
    return RIKKA_FLUID_SPREAD * fluid.radius * rikka_fluid_pixels_per_metre(view, distance);
}

float rikka_fluid_alike(float near, float distance, float steepest) {
    return near > 0.0 ? 1.0 - smoothstep(0.5 * steepest, steepest, abs(near - distance)) : 0.0;
}

float3 rikka_fluid_span(RikkaViewRef view, uint smoothed, int2 pixel, int2 toward, float distance,
                        float steepest) {
    int2 limit = int2(view.viewport) - 1;
    int2 ahead = clamp(pixel + toward, int2(0), limit);
    int2 behind = clamp(pixel - toward, int2(0), limit);
    if (any(ahead - behind != 2 * toward)) {
        return float3(0.0);
    }
    float ahead_distance = rikka_fetch(rikka_images[smoothed], ahead, 0).r;
    float behind_distance = rikka_fetch(rikka_images[smoothed], behind, 0).r;
    float weight = min(rikka_fluid_alike(ahead_distance, distance, steepest),
        rikka_fluid_alike(behind_distance, distance, steepest));
    if (weight <= 0.0) {
        return float3(0.0);
    }
    return weight * (rikka_fluid_position(view, (float2(ahead) + 0.5) * view.inverse_viewport, ahead_distance)
        - rikka_fluid_position(view, (float2(behind) + 0.5) * view.inverse_viewport, behind_distance));
}

float3 rikka_fluid_shading_normal(RikkaViewRef view, RikkaFluidRef fluid, int2 pixel, float3 position,
                                  float distance, float3 eye, float diffusing) {
    float3 across = rikka_fluid_step(view, fluid.smoothed, pixel, int2(1, 0), position, distance);
    float3 down = rikka_fluid_step(view, fluid.smoothed, pixel, int2(0, 1), position, distance);
    float3 fine = rikka_fluid_facing(across, down, eye);
    if (diffusing <= RIKKA_FLUID_CLEAR) {
        return fine;
    }
    int reach = clamp(int(RIKKA_FLUID_DIFFUSION * rikka_fluid_spread(view, fluid, distance)), 1,
        RIKKA_FLUID_DIFFUSION_REACH);
    float steepest = RIKKA_FLUID_STEEPEST * RIKKA_FLUID_DIFFUSION * fluid.radius;
    across += rikka_fluid_span(view, fluid.smoothed, pixel, int2(reach, 0), distance, steepest);
    down += rikka_fluid_span(view, fluid.smoothed, pixel, int2(0, reach), distance, steepest);
    return normalize(lerp(fine, rikka_fluid_facing(across, down, eye), diffusing));
}

float4 rikka_fluid_sample(RikkaViewRef view, uint image, float2 uv, uint divisor) {
    return rikka_images[NonUniformResourceIndex(image)].SampleLevel(
        rikka_samplers[RIKKA_FLUID_LINEAR_CLAMP], rikka_render_uv(view, uv, divisor), 0.0);
}

float4 rikka_fluid_thin_sample(RikkaViewRef view, uint image, float2 uv) {
    return rikka_fluid_sample(view, image, uv, RIKKA_FLUID_THICKNESS_DIVISOR);
}

float rikka_fluid_raw_distance(RikkaFrameRef frame, uint depth, int2 pixel) {
    if (rikka_fetch(rikka_images[frame.fluid.front], pixel, 0).a <= 0.0) {
        return 0.0;
    }
    return rikka_fluid_distance_of(frame.view.projection, rikka_fetch(rikka_images[depth], pixel, 0).r);
}

float4 rikka_fluid_amount(RikkaViewRef view, RikkaFluidRef fluid, uint image, float2 uv) {
    float mask = rikka_fluid_thin_sample(view, fluid.coverage, uv).r;
    return rikka_fluid_thin_sample(view, image, uv) / max(mask, RIKKA_FLUID_SPARSEST);
}

float4 rikka_fluid_front(RikkaViewRef view, RikkaFluidRef fluid, int2 pixel) {
    float4 front = rikka_fetch(rikka_images[fluid.front], pixel, 0);
    int2 limit = int2(view.viewport) - 1;
    for (int offset = 1; offset <= RIKKA_FLUID_FILL_REACH && front.a <= 0.0; ++offset) {
        for (int way = 0; way < 8 && front.a <= 0.0; ++way) {
            int2 near = clamp(pixel + RIKKA_FLUID_WAYS[way] * offset, int2(0), limit);
            front = rikka_fetch(rikka_images[fluid.front], near, 0);
        }
    }
    return front;
}

float3 rikka_fluid_outline(RikkaViewRef view, RikkaFluidRef fluid, float2 uv, float distance) {
    float spread = max(rikka_fluid_spread(view, fluid, distance), 1.0);
    float2 reach = RIKKA_FLUID_PROBE * spread * view.inverse_viewport;
    float right = rikka_fluid_thin_sample(view, fluid.coverage, uv + float2(reach.x, 0.0)).r;
    float left = rikka_fluid_thin_sample(view, fluid.coverage, uv - float2(reach.x, 0.0)).r;
    float below = rikka_fluid_thin_sample(view, fluid.coverage, uv + float2(0.0, reach.y)).r;
    float above = rikka_fluid_thin_sample(view, fluid.coverage, uv - float2(0.0, reach.y)).r;
    float broad = smoothstep(RIKKA_FLUID_BROAD.x, RIKKA_FLUID_BROAD.y,
        max(max(right, left), max(below, above)));
    float enclosed = smoothstep(RIKKA_FLUID_BROAD.x, RIKKA_FLUID_BROAD.y,
        max(min(right, left), min(below, above)));
    float softness = 0.4 / spread;
    float mask = rikka_fluid_thin_sample(view, fluid.coverage, uv).r;
    float outline = smoothstep(RIKKA_FLUID_OUTLINE - softness, RIKKA_FLUID_OUTLINE + softness, mask);
    return float3(outline, broad, enclosed);
}

float rikka_fluid_coverage(RikkaViewRef view, RikkaFluidRef fluid, float2 uv, float distance, float path) {
    float raw = smoothstep(RIKKA_FLUID_EDGE.x * fluid.radius, RIKKA_FLUID_EDGE.y * fluid.radius, path);
    float3 outline = rikka_fluid_outline(view, fluid, uv, distance);
    return raw * lerp(1.0, outline.x, outline.y * (1.0 - outline.z));
}
