import rikka.effect;

struct Parameters {
    [Default(6504.0)] [Range(1667.0, 25000.0)] [Label("temperature of the light (K)")] float temperature;
    [Default(0.0)] [Range(-0.05, 0.05)] [Label("tint of the light (Duv, + greenish)")] float tint;
};

struct Push {
    Parameters* parameters;
    uint source;
    uint target;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float NEUTRAL_TEMPERATURE = 6504.0;
static const float2 D65_XY = float2(0.31271, 0.32902);
static const float3x3 RGB_TO_XYZ = float3x3(
    0.4124564, 0.3575761, 0.1804375,
    0.2126729, 0.7151522, 0.0721750,
    0.0193339, 0.1191920, 0.9503041);
static const float3x3 XYZ_TO_RGB = float3x3(
    3.2404542, -1.5371385, -0.4985314,
    -0.9692660, 1.8760108, 0.0415560,
    0.0556434, -0.2040259, 1.0572252);
static const float3x3 BRADFORD = float3x3(
    0.8951, 0.2664, -0.1614,
    -0.7502, 1.7135, 0.0367,
    0.0389, -0.0685, 1.0296);
static const float3x3 BRADFORD_INVERSE = float3x3(
    0.9869929, -0.1470543, 0.1599627,
    0.4323053, 0.5183603, 0.0492912,
    -0.0085287, 0.0400428, 0.9684867);

float2 planckian(float kelvin) {
    float t = clamp(kelvin, 1000.0, 25000.0);
    float t2 = t * t;
    float u = (0.860117757 + 1.54118254e-4 * t + 1.28641212e-7 * t2) / (1.0 + 8.42420235e-4 * t + 7.08145163e-7 * t2);
    float v = (0.317398726 + 4.22806245e-5 * t + 4.20481691e-8 * t2) / (1.0 - 2.89741816e-5 * t + 1.61456053e-7 * t2);
    return float2(u, v);
}

float2 xy_to_uv(float2 xy) {
    float denominator = -2.0 * xy.x + 12.0 * xy.y + 3.0;
    return float2(4.0 * xy.x, 6.0 * xy.y) / denominator;
}

float2 uv_to_xy(float2 uv) {
    float denominator = 2.0 * uv.x - 8.0 * uv.y + 4.0;
    return float2(3.0 * uv.x, 2.0 * uv.y) / denominator;
}

float2 light(float kelvin, float tint) {
    float2 here = planckian(kelvin);
    float2 ahead = planckian(kelvin + 1.0);
    float2 along = normalize(ahead - here);
    float2 normal = float2(-along.y, along.x);
    if (normal.y < 0.0) {
        normal = -normal;
    }
    float2 shift = xy_to_uv(D65_XY) - planckian(NEUTRAL_TEMPERATURE);
    return here + normal * tint + shift;
}

float3 white(float2 uv) {
    float2 xy = uv_to_xy(uv);
    return float3(xy.x / xy.y, 1.0, (1.0 - xy.x - xy.y) / xy.y);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void adapt(uint3 id: SV_DispatchThreadID) {
    int2 pixel = int2(id.xy);
    if (rikka_effect_outside(id.xy, push.size)) {
        return;
    }
    Parameters parameters = *push.parameters;
    float4 color = rikka_effect_load(push.source, pixel);
    if (parameters.temperature == NEUTRAL_TEMPERATURE && parameters.tint == 0.0) {
        rikka_effect_store(push.target, pixel, color);
        return;
    }
    float3 source = mul(BRADFORD, white(light(parameters.temperature, parameters.tint)));
    float3 target = mul(BRADFORD, white(xy_to_uv(D65_XY)));
    float3 cone = mul(BRADFORD, mul(RGB_TO_XYZ, color.rgb)) * target / source;
    float3 adapted = mul(XYZ_TO_RGB, mul(BRADFORD_INVERSE, cone));
    rikka_effect_store(push.target, pixel, float4(adapted, color.a));
}
