import plane;

struct Push {
    Head head;
    float4 a;
    float4 b;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint CHROMA = 0u;
static const uint LUMA = 1u;
static const uint CLIP = 2u;

static const float3 REC709 = float3(0.2126, 0.7152, 0.0722);
static const float CB_SCALE = 1.8556;
static const float CR_SCALE = 1.5748;

float3 to_ycbcr(float3 c) {
    float y = dot(c, REC709);
    return float3(y, (c.b - y) / CB_SCALE, (c.r - y) / CR_SCALE);
}

float3 from_ycbcr(float3 ycc) {
    float r = ycc.x + CR_SCALE * ycc.z;
    float b = ycc.x + CB_SCALE * ycc.y;
    float g = (ycc.x - REC709.r * r - REC709.b * b) / REC709.g;
    return float3(r, g, b);
}

float ramp(float value, float from, float width) {
    if (width <= 0.0) {
        return value >= from ? 1.0 : 0.0;
    }
    return smoothstep(from, from + width, value);
}

float4 chroma(float4 c) {
    float3 ycc = to_ycbcr(c.rgb);
    float2 key = pc.a.yz;
    float distance = length(ycc.yz - key);
    float kept = ramp(distance, pc.b[0], pc.b[1]);
    float strength = length(key);
    if (strength > 1e-5 && pc.b[2] > 0.0) {
        float2 toward = key / strength;
        float along = dot(ycc.yz, toward);
        if (along > 0.0) {
            ycc.yz -= toward * along * pc.b[2];
        }
    }
    return float4(from_ycbcr(ycc), c.a * kept);
}

float4 luma(float4 c) {
    float y = dot(c.rgb, REC709);
    float bright = ramp(y, pc.a[0] - pc.a[1] * 0.5, pc.a[1]);
    float kept = pc.a[2] > 0.5 ? 1.0 - bright : bright;
    return float4(c.rgb, c.a * kept);
}

float cut(float inside, float band, float feather) {
    if (band <= 0.0) {
        return 1.0;
    }
    if (feather <= 0.0) {
        return clamp(inside + 0.5, 0.0, 1.0);
    }
    return smoothstep(0.0, feather, inside);
}

float4 clipped(float4 c, int2 pixel) {
    float2 size = float2(pc.head.dst.size());
    float2 p = float2(pixel) + 0.5;
    float feather = pc.b[0];
    float kept = cut(p.x - pc.a[0], pc.a[0], feather)
        * cut(size.x - pc.a[2] - p.x, pc.a[2], feather)
        * cut(p.y - pc.a[1], pc.a[1], feather)
        * cut(size.y - pc.a[3] - p.y, pc.a[3], feather);
    return float4(c.rgb, c.a * kept);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void key(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 c = pc.head.src.load(pixel);
    float4 result;
    if (pc.head.mode == CHROMA) {
        result = chroma(c);
    } else if (pc.head.mode == LUMA) {
        result = luma(c);
    } else {
        result = clipped(c, pixel);
    }
    pc.head.dst.store(pixel, pc.head.clamped(result));
}
