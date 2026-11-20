import plane;

struct Push {
    Head head;
    Plane other;
    float4 k;
    float4 ink;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint SHIFT = 0u;
static const uint SILHOUETTE = 1u;
static const uint UNDER = 2u;
static const uint SEED = 3u;
static const uint FLOOD = 4u;
static const uint RING = 5u;

static const float SEEDED = 1.0 / 255.0;

float4 outside_bilinear(Plane p, float2 at) {
    float2 corner = at - 0.5;
    int2 low = int2(floor(corner));
    float2 t = corner - float2(low);
    float4 values[4];
    for (int index = 0; index < 4; index++) {
        int2 tap = low + int2(index & 1, index >> 1);
        values[index] = p.contains(tap) ? p.load(tap) : float4(0.0);
    }
    return lerp(lerp(values[0], values[1], t.x), lerp(values[2], values[3], t.x), t.y);
}

float4 shifted(float2 center) {
    float4 red = outside_bilinear(pc.head.src, center - pc.k.xy);
    float4 green = pc.head.src.load(int2(floor(center)));
    float4 blue = outside_bilinear(pc.head.src, center + pc.k.xy);
    return float4(red.r, green.g, blue.b, max(green.a, max(red.a, blue.a)));
}

float reach(float4 seed, float2 center) {
    return distance(center, seed.xy) + 0.5 - seed.z;
}

float4 flooded(int2 pixel, float2 center) {
    int step = int(pc.k[0]);
    float4 best = float4(0.0);
    float nearest = 1e30;
    for (int y = -1; y <= 1; y++) {
        for (int x = -1; x <= 1; x++) {
            int2 tap = pixel + int2(x, y) * step;
            if (!pc.head.src.contains(tap)) {
                continue;
            }
            float4 seed = pc.head.src.load(tap);
            if (seed.w <= 0.0) {
                continue;
            }
            float d = reach(seed, center);
            if (d < nearest) {
                nearest = d;
                best = seed;
            }
        }
    }
    return best;
}

float ringed(float2 center) {
    float4 seed = pc.head.src.load(int2(floor(center)));
    if (seed.w <= 0.0) {
        return 0.0;
    }
    float d = reach(seed, center);
    float width = pc.k[0];
    float soft = max(pc.k[1], 1.0);
    float t = clamp((width + soft * 0.5 - d) / soft, 0.0, 1.0);
    return pc.k[1] > 1.0 ? smoothstep(0.0, 1.0, t) : t;
}

[shader("compute")]
[numthreads(16, 16, 1)]
void matte(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float2 center = float2(pixel) + 0.5;
    float4 result;
    if (pc.head.mode == SHIFT) {
        result = shifted(center);
    } else if (pc.head.mode == SILHOUETTE) {
        result = pc.ink * outside_bilinear(pc.head.src, center - pc.k.xy).a;
    } else if (pc.head.mode == UNDER) {
        float4 top = pc.head.src.load(pixel);
        result = top + pc.other.load(pixel) * (1.0 - top.a);
    } else if (pc.head.mode == SEED) {
        float alpha = clamp(pc.head.src.load(pixel).a, 0.0, 1.0);
        result = alpha >= SEEDED ? float4(center, alpha, 1.0) : float4(0.0);
    } else if (pc.head.mode == FLOOD) {
        result = flooded(pixel, center);
    } else {
        result = pc.ink * ringed(center);
    }
    pc.head.dst.store(pixel, result);
}
