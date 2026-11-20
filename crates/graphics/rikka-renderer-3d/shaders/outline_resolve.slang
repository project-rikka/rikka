import rikka.common;

struct Push {
    uint color;
    uint segments;
    uint smoothing;
    float line_kernel;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float DISC_RADIUS = 0.5641895835477563 * 1.05;
static const float LINE_SMOOTH_START = 0.5 - DISC_RADIUS;
static const float LINE_SMOOTH_END = 0.5 + DISC_RADIUS;
static const float NO_LINE = 1e10;

struct Texel {
    float4 color;
    float2 direction;
    float distance;
    bool valid;
};

Texel fetch(int2 texel) {
    int2 at = clamp(texel, int2(0), int2(push.size) - 1);
    float4 segment = rikka_images[push.segments].Load(int3(at, 0));
    Texel t;
    t.color = rikka_images[push.color].Load(int3(at, 0));
    t.direction = segment.xy;
    t.distance = segment.z;
    t.valid = segment.w > 0.5;
    return t;
}

float coverage(float distance_to_line) {
    return smoothstep(LINE_SMOOTH_END, LINE_SMOOTH_START, abs(distance_to_line) - push.line_kernel);
}

float neighbor_distance(Texel neighbor, int2 offset) {
    bool horizontal_direction = abs(neighbor.direction.x) > abs(neighbor.direction.y);
    bool horizontal_offset = offset.x != 0;
    if (!neighbor.valid || horizontal_direction != horizontal_offset) {
        return NO_LINE;
    }
    return neighbor.distance + dot(float2(offset), -neighbor.direction);
}

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    int2 pixel = int2(frag_coord.xy);
    Texel center = fetch(pixel);
    float4 color = center.color;
    if (push.smoothing != 0u) {
        if (center.valid) {
            color *= coverage(center.distance);
        }
        int2 offsets[4] = { int2(1, 0), int2(-1, 0), int2(0, 1), int2(0, -1) };
        for (uint side = 0u; side < 4u; side++) {
            Texel neighbor = fetch(pixel + offsets[side]);
            float covered = coverage(neighbor_distance(neighbor, offsets[side]));
            color += neighbor.color * covered * (1.0 - color.a);
        }
    }
    if (!(color.a > 0.0)) {
        discard;
    }
    return color;
}
