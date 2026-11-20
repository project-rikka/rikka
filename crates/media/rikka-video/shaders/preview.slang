struct Push {
    uint source;
    uint target;
    int2 size;
    int2 limit;
    int2 origin;
}

[[vk::binding(0, 0)]] Texture2D<float4> reads[];
[[vk::binding(1, 0)]] [format("unknown")] WTexture2D<float4> writes[];

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const int MOST_TAPS = 8;

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

[shader("compute")]
[numthreads(8, 8, 1)]
void shrink(uint3 id : SV_DispatchThreadID) {
    int2 at = int2(id.xy);
    if (at.x >= pc.size.x || at.y >= pc.size.y) {
        return;
    }
    float2 scale = float2(pc.limit) / float2(pc.size);
    float2 centre = (float2(at) + 0.5) * scale - 0.5;
    int2 taps = clamp(int2(ceil(scale)), int2(1), int2(MOST_TAPS));
    float2 pitch = scale / float2(taps);
    float2 first = centre - 0.5 * scale + 0.5 * pitch;
    float3 sum = float3(0.0);
    for (int y = 0; y < taps.y; y++) {
        for (int x = 0; x < taps.x; x++) {
            sum += bilinear(first + float2(x, y) * pitch);
        }
    }
    float3 rgb = sum / float(taps.x * taps.y);
    writes[NonUniformResourceIndex(pc.target)].Store(at, float4(rgb, 1.0));
}
