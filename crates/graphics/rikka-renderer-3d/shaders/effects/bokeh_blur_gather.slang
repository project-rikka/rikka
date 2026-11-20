import rikka.effects.bokeh_blur;

struct Push {
    Parameters* parameters;
    uint image;
    uint size_map;
    uint aperture;
    uint half_level;
    uint quarter_level;
    uint eighth_level;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float FINE_REACH = 16.0;
static const float MIDDLE_REACH = 32.0;

[shader("compute")]
[numthreads(8, 8, 1)]
void gather(uint3 id: SV_DispatchThreadID) {
    if (rikka_effect_outside(id.xy, push.size) || push.parameters.radius <= 0.0) {
        return;
    }
    Parameters parameters = *push.parameters;
    int2 pixel = int2(id.xy);
    float2 size = float2(push.size);
    float2 uv = rikka_effect_uv(pixel, push.size);
    uint2 full = rikka_effect_image_size(push.image);
    float reach = 0.5 * rikka_bokeh_radius(push.parameters, push.size_map, uv, full);
    if (reach < 0.5) {
        rikka_effect_store(push.target, pixel, rikka_effect_load(push.half_level, pixel));
        return;
    }
    uint source = push.half_level;
    float spacing = 2.0;
    if (reach > MIDDLE_REACH) {
        source = push.eighth_level;
        spacing = 8.0;
    } else if (reach > FINE_REACH) {
        source = push.quarter_level;
        spacing = 4.0;
    }
    int rings = clamp(int(ceil(reach / spacing)), 1, RIKKA_BOKEH_MAX_RINGS);
    float squeeze = max(parameters.squeeze, 0.05);
    float2 stretch = float2(sqrt(squeeze), 1.0 / sqrt(squeeze));
    float4 sum = float4(0.0);
    float total = 0.0;
    for (int ring = 0; ring <= rings; ++ring) {
        int taps = max(ring * 8, 1);
        float rho = float(ring) / float(rings);
        float edge = ring == rings ? 0.5 : 1.0;
        for (int tap = 0; tap < taps; ++tap) {
            float angle = (float(tap) + 0.5 * float(ring & 1)) * RIKKA_TAU / float(taps);
            float2 unit = rho * float2(cos(angle), sin(angle));
            float weight;
            float2 offset;
            if (push.aperture != RIKKA_NONE) {
                float2 kernel = rikka_effect_rotate(-unit, -parameters.rotation) * 0.5 + 0.5;
                float3 shape = rikka_effect_sample(push.aperture, kernel).rgb;
                weight = max(rikka_luma(shape), 0.0);
                offset = unit;
            } else {
                float polygon = rikka_bokeh_polygon(angle + RIKKA_PI, parameters.blades, parameters.rotation, parameters.curvature);
                weight = polygon * polygon;
                offset = unit * polygon;
            }
            offset *= stretch * reach;
            float4 color = rikka_effect_sample(source, (float2(pixel) + 0.5 + offset) / size);
            sum += color * weight * edge;
            total += weight * edge;
        }
    }
    float4 blurred = total > 1e-6 ? sum / total : rikka_effect_load(push.half_level, pixel);
    rikka_effect_store(push.target, pixel, blurred);
}
