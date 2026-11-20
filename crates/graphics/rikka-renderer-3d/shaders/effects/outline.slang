import rikka.effect;

struct Fade {
    [Default(2.0)] [Range(0.0, 1000.0)] float near;
    [Default(40.0)] [Range(0.0, 10000.0)] float far;
    [Default(0.35)] [Range(0.0, 1.0)] [Label("width at far")] float thinnest;
};

struct Parameters {
    [Default(2.0)] [Range(0.0, 8.0)] [Label("width (px at 1080p)")] float width;
    [Default(0.02, 0.016, 0.025)] [Color] float3 color;
    [Default(1.0)] [Range(0.0, 1.0)] float opacity;
    [Default(0.25)] [Range(0.0, 1.0)] [Label("scene tint")] float tint;
    [Default(0.06)] [Range(0.001, 1.0)] [Label("depth threshold")] float depth;
    [Default(0.45)] [Range(0.0, 2.0)] [Label("crease threshold")] float crease;
    [Default(1)] uint mask;
    [Default(1.0)] bool silhouettes;
    [Group("distance")] Fade fade;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint depth;
    uint normals;
    uint layers;
    uint target;
    uint2 size;
    RikkaFrameRef frame;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int DIRECTIONS = 8;
static const float2 OFFSETS[8] = {
    float2(1.0, 0.0), float2(-1.0, 0.0), float2(0.0, 1.0), float2(0.0, -1.0),
    float2(0.7071, 0.7071), float2(-0.7071, 0.7071), float2(0.7071, -0.7071), float2(-0.7071, -0.7071)
};
static const float SKY = 1e8;

struct Sample {
    float depth;
    float3 normal;
    uint layers;
};

Sample sample_at(int2 pixel) {
    int2 at = clamp(pixel, int2(0), int2(push.size) - 1);
    Sample s;
    s.depth = rikka_effect_linear_depth(push.frame, rikka_effect_load(push.depth, at).r);
    s.normal = rikka_effect_load(push.normals, at).xyz;
    s.layers = rikka_effect_load_layers(push.layers, at);
    return s;
}

float fade_width(Fade fade, float depth) {
    float t = smoothstep(fade.near, max(fade.far, fade.near + 1e-3), depth);
    return lerp(1.0, fade.thinnest, t);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void trace(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    Sample centre = sample_at(pixel);
    float base = parameters.width * rikka_effect_scale(push.size);
    float ink = 0.0;
    int2 owner = pixel;
    for (int ring = 1; ring <= 2; ++ring) {
        for (int direction = 0; direction < DIRECTIONS; ++direction) {
            float2 stride = OFFSETS[direction] * base * (float(ring) * 0.5);
            int2 offset = int2(round(stride));
            if (all(offset == int2(0))) {
                offset = int2(sign(OFFSETS[direction]));
            }
            Sample other = sample_at(pixel + offset);
            bool other_front = other.depth < centre.depth;
            Sample front = other_front ? other : centre;
            Sample back = other_front ? centre : other;
            if ((front.layers & parameters.mask) == 0u || front.depth >= SKY) {
                continue;
            }
            float reach = base * fade_width(parameters.fade, front.depth);
            float span = length(float2(offset));
            float coverage = clamp(reach + 1.0 - span, 0.0, 1.0);
            if (coverage <= 0.0) {
                continue;
            }
            float gap = (back.depth - front.depth) / max(front.depth, 1e-4);
            float silhouette = back.depth >= SKY ? (parameters.silhouettes ? 1.0 : 0.0)
                : smoothstep(parameters.depth, parameters.depth * 1.5, gap);
            float edge = other_front ? silhouette : 0.0;
            if (back.depth < SKY && gap < parameters.depth && (centre.layers & parameters.mask) != 0u) {
                float bend = 1.0 - dot(normalize(centre.normal + 1e-6), normalize(other.normal + 1e-6));
                edge = max(edge, smoothstep(parameters.crease * 0.7, parameters.crease * 1.5 + 0.01, bend));
            }
            if (edge * coverage > ink) {
                ink = edge * coverage;
                owner = other_front ? pixel + offset : pixel;
            }
        }
    }
    float3 straight = rikka_effect_straight(color);
    float3 under = rikka_effect_straight(rikka_effect_load_clamped(push.source, owner, push.size));
    float3 line_color = lerp(rikka_effect_encode(parameters.color), under * 0.35, clamp(parameters.tint, 0.0, 1.0));
    float matte = ink * clamp(parameters.opacity, 0.0, 1.0);
    float alpha = max(color.a, matte);
    float3 shown = lerp(straight * color.a, line_color, matte);
    rikka_effect_store(push.target, pixel, float4(shown, alpha));
}
