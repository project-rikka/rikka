import plane;

struct Push {
    Head head;
    float* carried;
    uint levels;
    uint pad;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint ORDERED = 0u;
static const uint DIFFUSED = 1u;
static const int BAND = 256;

static const float BAYER[64] = {
    0.0, 32.0, 8.0, 40.0, 2.0, 34.0, 10.0, 42.0,
    48.0, 16.0, 56.0, 24.0, 50.0, 18.0, 58.0, 26.0,
    12.0, 44.0, 4.0, 36.0, 14.0, 46.0, 6.0, 38.0,
    60.0, 28.0, 52.0, 20.0, 62.0, 30.0, 54.0, 22.0,
    3.0, 35.0, 11.0, 43.0, 1.0, 33.0, 9.0, 41.0,
    51.0, 19.0, 59.0, 27.0, 49.0, 17.0, 57.0, 25.0,
    15.0, 47.0, 7.0, 39.0, 13.0, 45.0, 5.0, 37.0,
    63.0, 31.0, 55.0, 23.0, 61.0, 29.0, 53.0, 21.0,
};

float load_shared(float* at) {
    return loadCoherent<4, MemoryScope.Workgroup>(at);
}

void store_shared(float* at, float value) {
    storeCoherent<4, MemoryScope.Workgroup>(at, value);
}

float3 quantized(float3 v) {
    float steps = float(pc.levels - 1u);
    return clamp(floor(v * steps + 0.5) / steps, 0.0, 1.0);
}

void ordered(uint3 id) {
    int2 pixel = pc.head.pixel(id);
    if (!pc.head.dst.contains(pixel)) {
        return;
    }
    float4 c = pc.head.src.load(pixel);
    float threshold = (BAYER[(pixel.y & 7) * 8 + (pixel.x & 7)] + 0.5) / 64.0 - 0.5;
    float steps = float(pc.levels - 1u);
    c.rgb = quantized(c.rgb + threshold / steps);
    pc.head.dst.store(pixel, c);
}

uint slot(int row, int x) {
    return (uint(row) * pc.head.dst.width + uint(x)) * 3u;
}

float3 carried_at(int row, int x) {
    if (x < 0 || x >= int(pc.head.dst.width)) {
        return float3(0.0);
    }
    uint at = slot(row, x);
    return float3(load_shared(pc.carried + at), load_shared(pc.carried + (at + 1u)),
                  load_shared(pc.carried + (at + 2u)));
}

void carry(int row, int x, float3 error) {
    if (x < 0 || x >= int(pc.head.dst.width)) {
        return;
    }
    uint at = slot(row, x);
    float* target = pc.carried;
    float previous0 = load_shared(target + at);
    float previous1 = load_shared(target + (at + 1u));
    float previous2 = load_shared(target + (at + 2u));
    store_shared(target + at, previous0 + error.r);
    store_shared(target + (at + 1u), previous1 + error.g);
    store_shared(target + (at + 2u), previous2 + error.b);
}

void store_carried(uint at, float3 value) {
    float* target = pc.carried;
    store_shared(target + at, value.r);
    store_shared(target + (at + 1u), value.g);
    store_shared(target + (at + 2u), value.b);
}

void diffused(uint lane_index) {
    int lane = int(lane_index);
    int width = int(pc.head.dst.width);
    int height = int(pc.head.dst.height);
    for (int band = 0; band < height; band += BAND) {
        int y = band + lane;
        int row = lane;
        float3 ahead = float3(0.0);
        int steps = width + 2 * BAND;
        for (int t = 0; t < steps; t++) {
            int x = t - 2 * lane;
            if (y < height && x >= 0 && x < width) {
                int2 pixel = int2(x, y);
                float4 c = pc.head.src.load(pixel);
                float3 wanted = c.rgb + ahead + carried_at(row, x);
                float3 chosen = quantized(wanted);
                float3 error = wanted - chosen;
                pc.head.dst.store(pixel, float4(chosen, c.a));
                ahead = error * (7.0 / 16.0);
                carry(row + 1, x - 1, error * (3.0 / 16.0));
                carry(row + 1, x, error * (5.0 / 16.0));
                carry(row + 1, x + 1, error * (1.0 / 16.0));
            }
            DeviceMemoryBarrierWithGroupSync();
        }
        for (int x = lane; x < width; x += BAND) {
            float3 last = carried_at(BAND, x);
            store_carried(slot(0, x), last);
            store_carried(slot(BAND, x), float3(0.0));
        }
        DeviceMemoryBarrierWithGroupSync();
        for (int r = 1; r < BAND; r++) {
            for (int x = lane; x < width; x += BAND) {
                store_carried(slot(r, x), float3(0.0));
            }
        }
        DeviceMemoryBarrierWithGroupSync();
    }
}

[shader("compute")]
[numthreads(256, 1, 1)]
void dither(uint3 id : SV_DispatchThreadID, uint3 local : SV_GroupThreadID) {
    if (pc.head.mode == ORDERED) {
        ordered(id);
    } else {
        diffused(local.x);
    }
}
