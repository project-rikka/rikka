struct Push {
    uint source;
    uint luma;
    uint chroma;
    uint unused;
    int2 size;
    int2 limit;
    int2 offset;
    int2 extent;
    int2 surface;
    int2 origin;
}

[[vk::binding(0, 0)]] Texture2D<float4> reads[];
[[vk::binding(1, 0)]] [format("unknown")] WTexture2D<float4> writes[];

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const float3 LUMA = float3(0.2126, 0.7152, 0.0722);
static const float LUMA_FOOT = 16.0 / 255.0;
static const float LUMA_SPAN = 219.0 / 255.0;
static const float CHROMA_MIDDLE = 128.0 / 255.0;
static const float CHROMA_SPAN = 224.0 / 255.0;
static const float BLUE_REACH = 1.8556;
static const float RED_REACH = 1.5748;

static const int MOST_TAPS = 4;

float3 fetched(int2 at) {
    int2 inside = clamp(at, int2(0), pc.limit - 1) + pc.origin;
    return reads[NonUniformResourceIndex(pc.source)].Load(int3(inside, 0)).rgb;
}

float3 bilinear(float2 from) {
    int2 low = int2(floor(from));
    float2 t = from - float2(low);
    float3 top = lerp(fetched(low), fetched(low + int2(1, 0)), t.x);
    float3 bottom = lerp(fetched(low + int2(0, 1)), fetched(low + int2(1, 1)), t.x);
    return lerp(top, bottom, t.y);
}

float3 placed(int2 at) {
    int2 within = at - pc.offset;
    if (any(within < int2(0)) || any(within >= pc.extent)) {
        return float3(0.0);
    }
    int2 spare = pc.limit - pc.extent;
    if (all(spare >= int2(0)) && all(spare <= int2(1))) {
        return fetched(within);
    }
    float2 scale = float2(pc.limit) / float2(pc.extent);
    float2 centre = (float2(within) + 0.5) * scale - 0.5;
    int2 taps = clamp(int2(ceil(scale)), int2(1), int2(MOST_TAPS));
    float2 pitch = scale / float2(taps);
    float2 first = centre - 0.5 * scale + 0.5 * pitch;
    float3 sum = float3(0.0);
    for (int y = 0; y < taps.y; y++) {
        for (int x = 0; x < taps.x; x++) {
            sum += bilinear(first + float2(x, y) * pitch);
        }
    }
    return sum / float(taps.x * taps.y);
}

[shader("compute")]
[numthreads(8, 8, 1)]
void convert(uint3 id : SV_DispatchThreadID) {
    int2 block = int2(id.xy);
    int2 origin = block * 2;
    if (origin.x >= pc.surface[0] || origin.y >= pc.surface[1]) {
        return;
    }
    float3 sum = float3(0.0);
    for (int dy = 0; dy < 2; dy++) {
        for (int dx = 0; dx < 2; dx++) {
            int2 at = origin + int2(dx, dy);
            float3 rgb = placed(min(at, pc.size - 1));
            float luma = dot(rgb, LUMA);
            writes[NonUniformResourceIndex(pc.luma)].Store(at, float4(LUMA_FOOT + luma * LUMA_SPAN));
            sum += rgb;
        }
    }
    float3 mean = sum * 0.25;
    float luma = dot(mean, LUMA);
    float blue = CHROMA_MIDDLE + (mean.b - luma) / BLUE_REACH * CHROMA_SPAN;
    float red = CHROMA_MIDDLE + (mean.r - luma) / RED_REACH * CHROMA_SPAN;
    writes[NonUniformResourceIndex(pc.chroma)].Store(block, float4(blue, red, 0.0, 0.0));
}
