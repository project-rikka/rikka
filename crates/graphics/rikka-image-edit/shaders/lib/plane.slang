[[vk::binding(0, 0)]] Texture2D<float4> plane_reads[];
[[vk::binding(1, 0)]] [format("unknown")] WTexture2D<float4> plane_writes[];

static const uint SIDE_SHIFT = 16u;
static const uint ACROSS_MASK = 65535u;

static const uint FLAG_CLAMP = 1u;

struct Plane {
    uint base;
    uint across;
    uint width;
    uint height;

    int2 size() {
        return int2(int(width), int(height));
    }

    bool contains(int2 at) {
        return at.x >= 0 && at.y >= 0 && at.x < int(width) && at.y < int(height);
    }

    uint shift() {
        return across >> SIDE_SHIFT;
    }

    uint tile(uint2 at) {
        uint2 tile = at >> shift();
        return base + tile.y * (across & ACROSS_MASK) + tile.x;
    }

    int2 texel(uint2 at) {
        return int2(at & ((1u << shift()) - 1u));
    }

    float4 load(int2 at) {
        uint2 inside = uint2(clamp(at, int2(0), size() - 1));
        return plane_reads[NonUniformResourceIndex(tile(inside))].Load(int3(texel(inside), 0));
    }

    void store(int2 at, float4 value) {
        uint2 inside = uint2(at);
        plane_writes[NonUniformResourceIndex(tile(inside))].Store(uint2(texel(inside)), value);
    }

    float4 bilinear(float2 at) {
        float2 corner = at - 0.5;
        int2 low = int2(floor(corner));
        float2 t = corner - float2(low);
        float4 a = load(low);
        float4 b = load(low + int2(1, 0));
        float4 c = load(low + int2(0, 1));
        float4 d = load(low + int2(1, 1));
        return lerp(lerp(a, b, t.x), lerp(c, d, t.x), t.y);
    }

    float4 bicubic(float2 at) {
        float2 corner = at - 0.5;
        int2 low = int2(floor(corner));
        float2 t = corner - float2(low);
        float4 sum = float4(0.0);
        float total = 0.0;
        for (int y = -1; y <= 2; y++) {
            float wy = cubic_weight(float(y) - t.y);
            for (int x = -1; x <= 2; x++) {
                float w = wy * cubic_weight(float(x) - t.x);
                sum += w * load(low + int2(x, y));
                total += w;
            }
        }
        return sum / total;
    }
}

struct Head {
    Plane src;
    Plane dst;
    int2 origin;
    uint flags;
    uint mode;
    int2 window;
    uint2 head_pad;

    int2 pixel(uint3 id) {
        return int2(id.xy) + origin;
    }

    float4 clamped(float4 value) {
        return (flags & FLAG_CLAMP) != 0u ? clamp(value, 0.0, 1.0) : value;
    }
}

float cubic_weight(float x) {
    x = abs(x);
    const float a = -0.5;
    if (x <= 1.0) {
        return ((a + 2.0) * x - (a + 3.0)) * x * x + 1.0;
    }
    if (x < 2.0) {
        return ((a * x - 5.0 * a) * x + 8.0 * a) * x - 4.0 * a;
    }
    return 0.0;
}

float2 mirror_wrap(float2 at, float2 size) {
    float2 twice = size * 2.0;
    float2 wrapped = at - twice * floor(at / twice);
    return size - abs(wrapped - size);
}

float hash01(uint3 v) {
    v = v * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v ^= v >> 16u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return float(v.x >> 8u) * (1.0 / 16777216.0);
}
