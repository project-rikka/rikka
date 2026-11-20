__exported import rikka.material;
__exported import rikka.characters;
__exported import rikka.pbr.brdf;

static const float RIKKA_NPR_HEAD_RADIUS = 0.1;

float rikka_npr_linearstep(float low, float high, float value) {
    return clamp((value - low) / max(high - low, 1e-5), 0.0, 1.0);
}

float rikka_npr_half(float x) {
    return x * 0.5 + 0.5;
}

float rikka_npr_channel(float4 value, float channel, float fallback) {
    int picked = int(channel);
    return picked <= 0 ? fallback : value[min(picked, 4) - 1];
}

float rikka_npr_pick(float4 value, float channel, float fallback, float invert) {
    float picked = rikka_npr_channel(value, channel, fallback);
    return invert > 0.5 ? 1.0 - picked : picked;
}

float rikka_npr_focal(RikkaContext context) {
    return max(abs(context.view.projection[1][1]), 1e-5);
}

float3 rikka_npr_to_view(RikkaContext context, float3 direction) {
    return mul((float3x3)context.view.view, direction);
}

float3 rikka_npr_from_view(RikkaContext context, float3 direction) {
    return mul(transpose((float3x3)context.view.view), direction);
}

float rikka_npr_depth(RikkaContext context, float3 position) {
    return dot(position - context.view.camera_position, context.view.camera_forward);
}

struct RikkaNprHead {
    float3 center;
    float radius;
    float3 forward;
    float3 up;
    float3 right;
    float present;
    float scale;
};

RikkaNprHead rikka_npr_head(RikkaContext context) {
    RikkaFigure figure = context.figure;
    RikkaNprHead head;
    head.center = figure.head_center;
    head.radius = figure.head_radius;
    head.present = figure.has_head != 0u ? 1.0 : 0.0;
    if (figure.has_head == 0u) {
        head.center = context.instance.center;
        head.radius = context.instance.radius;
        head.forward = float3(0.0, 0.0, 1.0);
        head.up = float3(0.0, 1.0, 0.0);
        head.right = float3(1.0, 0.0, 0.0);
        head.scale = 1.0;
        return head;
    }
    head.forward = normalize(figure.head_forward);
    head.up = normalize(figure.head_up - head.forward * dot(figure.head_up, head.forward));
    head.right = cross(head.up, head.forward);
    head.scale = max(figure.head_radius, 1e-4) / RIKKA_NPR_HEAD_RADIUS;
    return head;
}

struct RikkaNprHeadLight {
    float3 flat;
    float forward;
    float right;
    float facing;
    float on_right;
    float upright;
};

RikkaNprHeadLight rikka_npr_head_light(RikkaContext context, float3 light) {
    RikkaNprHead head = rikka_npr_head(context);
    float3 flat = light - head.up * dot(light, head.up);
    float length_of = length(flat);
    RikkaNprHeadLight h;
    h.upright = smoothstep(1e-3, 0.25, length_of);
    h.flat = length_of > 1e-5 ? flat / length_of : head.forward;
    h.forward = dot(head.forward, h.flat);
    h.right = dot(head.right, h.flat);
    h.facing = rikka_npr_half(h.forward);
    h.on_right = h.right > 0.0 ? 1.0 : 0.0;
    return h;
}

float3 rikka_npr_transform(RikkaContext context, float3 value, float mode, float point) {
    bool positional = point > 0.5;
    if (int(mode) == 1) {
        float3 world = rikka_npr_from_view(context, value);
        return positional ? world + context.view.camera_position : world;
    }
    return positional ? mul(context.view.view, float4(value, 1.0)).xyz : rikka_npr_to_view(context, value);
}
