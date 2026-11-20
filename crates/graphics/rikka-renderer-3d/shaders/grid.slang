import rikka.common;

struct Push {
    RikkaFrameRef frame;
    float3 color;
    float opacity;
    float spacing;
    float reach;
    uint vertical_axis;
};

[[vk::push_constant]] ConstantBuffer<Push> rikka_push;

static const float AXIS_EMPHASIS = 2.1;
static const float3 X_AXIS = float3(0.85, 0.25, 0.3);
static const float3 Y_AXIS = float3(0.3, 0.75, 0.35);
static const float3 Z_AXIS = float3(0.25, 0.45, 0.9);

struct Layer {
    float4 color;
    float depth;
};

float3 unproject(RikkaViewRef view, float2 ndc, float depth) {
    float4 point = mul(view.inverse_view_projection, float4(ndc, depth, 1.0));
    return point.xyz / point.w;
}

float line_coverage(float2 position, float2 footprint, float spacing) {
    float2 safe = max(footprint, float2(1e-6));
    float2 distance_to_line = abs(fract(position / spacing - 0.5) - 0.5) * spacing / safe;
    float2 line = 1.0 - min(distance_to_line, 1.0);
    float2 resolved = 1.0 - smoothstep(0.2, 0.5, footprint / spacing);
    line *= resolved;
    return max(line.x, line.y);
}

Layer ground_layer(float3 origin, float3 direction, RikkaViewRef view) {
    float rise = abs(direction.y) > 1e-6 ? direction.y : 1e-6;
    float t = -origin.y / rise;
    float3 world = origin + direction * t;
    float2 footprint = float2(
        length(float2(ddx(world.x), ddy(world.x))),
        length(float2(ddx(world.z), ddy(world.z))));

    Layer none = { float4(0.0), 0.0 };
    float reach = rikka_push.reach;
    bool resolvable = max(footprint.x, footprint.y) < rikka_push.spacing;
    bool inside = all(abs(world.xz) <= float2(reach) + footprint);
    if (!(t > 0.0) || !resolvable || !inside) {
        return none;
    }

    float line_alpha = rikka_push.opacity;
    float axis_alpha = min(line_alpha * AXIS_EMPHASIS, 1.0);
    float3 color = rikka_push.color;
    float alpha = line_coverage(world.xz, footprint, rikka_push.spacing) * line_alpha;

    float2 axis_distance = abs(world.xz) / max(footprint, float2(1e-6));
    float x_axis = 1.0 - min(axis_distance.y, 1.0);
    float z_axis = 1.0 - min(axis_distance.x, 1.0);
    if (x_axis > 0.0) {
        color = X_AXIS;
        alpha = max(alpha, x_axis * axis_alpha);
    }
    if (z_axis > 0.0) {
        color = Z_AXIS;
        alpha = max(alpha, z_axis * axis_alpha);
    }

    float4 clip = mul(view.view_projection, float4(world, 1.0));
    Layer layer = { float4(color * alpha, alpha), clip.z / clip.w };
    return layer;
}

Layer vertical_axis_layer(float2 pixel, RikkaViewRef view) {
    Layer none = { float4(0.0), 0.0 };
    if (rikka_push.vertical_axis == 0u) {
        return none;
    }
    float4 foot = mul(view.view_projection, float4(0.0, 0.0, 0.0, 1.0));
    float4 head = mul(view.view_projection, float4(0.0, rikka_push.reach, 0.0, 1.0));

    float foot_ahead = foot.w - foot.z;
    float head_ahead = head.w - head.z;
    if (foot_ahead < 0.0 && head_ahead < 0.0) {
        return none;
    }
    if (foot_ahead < 0.0) {
        foot = lerp(foot, head, foot_ahead / (foot_ahead - head_ahead));
    } else if (head_ahead < 0.0) {
        head = lerp(head, foot, head_ahead / (head_ahead - foot_ahead));
    }
    if (!(foot.w > 0.0) || !(head.w > 0.0)) {
        return none;
    }

    float2 from = (foot.xy / foot.w * 0.5 + 0.5) * view.viewport;
    float2 to = (head.xy / head.w * 0.5 + 0.5) * view.viewport;
    float2 along = to - from;
    float span = dot(along, along);
    float s = span > 1e-6 ? clamp(dot(pixel - from, along) / span, 0.0, 1.0) : 0.0;
    float coverage = 1.0 - min(length(pixel - (from + along * s)), 1.0);
    if (!(coverage > 0.0)) {
        return none;
    }

    float alpha = coverage * min(rikka_push.opacity * AXIS_EMPHASIS, 1.0);
    float depth = lerp(foot.z / foot.w, head.z / head.w, s);
    Layer layer = { float4(Y_AXIS * alpha, alpha), depth };
    return layer;
}

[shader("fragment")]
float4 fragment_main([[vk::location(0)]] float2 uv, out float frag_depth: SV_Depth) : SV_Target {
    RikkaViewRef view = rikka_push.frame.view;
    float2 ndc = uv * 2.0 - 1.0;
    float3 origin = unproject(view, ndc, 1.0);
    float3 direction = normalize(unproject(view, ndc, 0.5) - origin);

    Layer ground = ground_layer(origin, direction, view);
    Layer axis = vertical_axis_layer(uv * view.viewport, view);

    bool axis_in_front = axis.depth >= ground.depth;
    Layer front = axis_in_front ? axis : ground;
    Layer back = axis_in_front ? ground : axis;
    float4 color = front.color + back.color * (1.0 - front.color.a);
    if (!(color.a > 0.002)) {
        discard;
    }

    float depth = front.color.a > 0.002 ? front.depth : back.depth;
    if (!(depth >= 0.0 && depth <= 1.0)) {
        discard;
    }
    frag_depth = depth;
    return color;
}
