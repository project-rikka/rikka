import rikka.space.view;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint sector;
    float2 light;
    float march;
    int first_block;
    float4 along;
    float4 depth;
    float4 toward;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const uint WAVE = 64u;
static const uint SAMPLES = 32u;
static const uint SPAN = WAVE + SAMPLES;
static const uint HARD = 4u;
static const float FADE_START = 0.6;
static const float BLEND_LIMIT = 0.02;
static const uint NEAREST_CLAMP = 3u;
static const int ROW_MARGIN = 1;
static const int64_t FIXED = 64;

groupshared float depths[SPAN];

struct Sector {
    uint major;
    float side;
};

Sector sector_of(uint index) {
    Sector s;
    s.major = index < 2u ? 0u : 1u;
    s.side = (index & 1u) == 0u ? 1.0 : -1.0;
    return s;
}

int2 pixel_of(Sector s, int along, int across) {
    return s.major == 0u ? int2(along, across) : int2(across, along);
}

struct Line {
    float base;
    float far_u;
    float far_v;
};

float column_u(Sector s, Line line, int column) {
    float centre = line.base + s.side * float(column) + 0.5;
    float light_major = s.major == 0u ? push.light.x : push.light.y;
    return max(s.side * (centre - light_major), 1e-3);
}

bool rows(Sector s, float far_u, float near_u, float2 size, out float lowest, out float highest) {
    float light_minor = s.major == 0u ? push.light.y : push.light.x;
    float extent = s.major == 0u ? size.y : size.x;
    float lo = -light_minor;
    float hi = extent - light_minor;
    float shrink = near_u / far_u;
    lowest = max(lo < 0.0 ? lo / shrink : lo, -far_u);
    highest = min(hi > 0.0 ? hi / shrink : hi, far_u);
    return lowest <= highest;
}

float view_distance(float depth) {
    float denominator = depth * push.depth.y - push.depth.x;
    return abs(denominator) < 1e-12 ? 1e30 : (depth * push.depth.w - push.depth.z) / denominator;
}

float ray_depth(float4 origin, float4 along, uint major, float coordinate) {
    float origin_m = major == 0u ? origin.x : origin.y;
    float along_m = major == 0u ? along.x : along.y;
    float denominator = along_m - coordinate * along.w;
    if (abs(denominator) < 1e-12) {
        return -1.0;
    }
    float t = (coordinate * origin.w - origin_m) / denominator;
    float4 clip = origin + along * t;
    return clip.w > 0.0 ? clip.z / clip.w : -1.0;
}

struct Fan {
    int64_t light_minor;
    int64_t u;
    int64_t far_u;
};

int64_t to_fixed(float value) {
    return int64_t(round(value * float(FIXED)));
}

int64_t fixed_u(Sector s, int64_t light_major, int along) {
    return max(int64_t(s.side) * (int64_t(along) * FIXED + FIXED / 2 - light_major), int64_t(1));
}

int64_t floor_div(int64_t value, int64_t divisor) {
    int64_t quotient = value / divisor;
    return quotient * divisor > value ? quotient - 1 : quotient;
}

int crossing(Fan fan, int row) {
    int64_t offset = int64_t(row) * FIXED + FIXED / 2 - fan.light_minor;
    return int(floor_div(fan.light_minor * fan.far_u + offset * fan.u, FIXED * fan.far_u));
}

int sector_floor(int64_t light_major, int64_t light_minor, int along) {
    int64_t reach = abs(int64_t(along) * FIXED + FIXED / 2 - light_major);
    return int(floor_div(light_minor - reach + FIXED / 2 - 1, FIXED));
}

bool in_sector(Sector s, int2 pixel) {
    int64_t dx = abs(int64_t(pixel.x) * FIXED + FIXED / 2 - to_fixed(push.light.x));
    int64_t dy = abs(int64_t(pixel.y) * FIXED + FIXED / 2 - to_fixed(push.light.y));
    return s.major == 0u ? dy <= dx : dx < dy;
}

struct Receiver {
    int column;
    int along;
    int minor;
    float v;
    float raw;
    float3 normal;
    float line_depth;
    uint own;
};

float march(Sector s, Line line, float2 size, Receiver pixel) {
    if (pixel.raw <= 0.0 || dot(pixel.normal, push.toward.xyz) <= 0.0) {
        return 0.0;
    }
    RikkaSpace space = push.frame.look.space;
    bool toward = push.march > 0.0;
    uint own = pixel.own;
    float2 on_line = float2(float(pixel.along) + 0.5, push.light[1u - s.major] + pixel.v);
    float2 centred = float2(float(pixel.along) + 0.5, float(pixel.minor) + 0.5);
    bool smooth = pixel.line_depth > 0.0;
    float2 at = smooth ? on_line : centred;
    at = s.major == 0u ? at : at.yx;
    float depth = smooth ? pixel.line_depth : pixel.raw;
    float2 ndc = at / size * 2.0 - 1.0;
    float receiver = view_distance(depth);
    float w = -push.depth.y * receiver + push.depth.w;
    float4 along_clip = push.along;
    float4 origin = float4(ndc * w, depth * w, w) + along_clip * (0.0015 * receiver + 0.002);
    float thickness = space.screen_shadow_thickness * (1.0 + 0.05 * receiver);
    float extent = s.major == 0u ? size.x : size.y;
    int step_column = toward ? -1 : 1;
    float here = (float(pixel.along) + 0.5) / extent * 2.0 - 1.0;
    float next = (float(int(line.base) + int(s.side) * (pixel.column + step_column)) + 0.5) / extent * 2.0 - 1.0;
    float start = ray_depth(origin, along_clip, s.major, here);
    float slope = ray_depth(origin, along_clip, s.major, next) - start;
    float shadow = 0.0;
    [unroll]
    for (uint step = 1u; step <= SAMPLES; ++step) {
        uint index = toward ? own - step : own + step;
        float sample_depth = abs(depths[index]);
        float expected = start + slope * float(step);
        if (sample_depth > expected && expected > 0.0) {
            float gap = view_distance(expected) - view_distance(sample_depth);
            float weight = step <= HARD
                ? 1.0
                : 1.0 - smoothstep(FADE_START, 1.0, float(step) / float(SAMPLES));
            shadow = gap <= thickness ? max(shadow, weight) : shadow;
        }
    }
    return shadow;
}

[shader("compute")]
[numthreads(64, 1, 1)]
void compute_main(uint3 group: SV_GroupID, uint lane: SV_GroupIndex) {
    RikkaFrameRef frame = push.frame;
    RikkaViewRef view = frame.view;
    Texture2D depth_image = rikka_images[frame.look.scene.depth];
    float2 size = view.viewport;
    Sector s = sector_of(push.sector);
    float light_major = s.major == 0u ? push.light.x : push.light.y;
    float light_minor = s.major == 0u ? push.light.y : push.light.x;

    Line line;
    line.base = floor(light_major + 0.5 * s.side);
    int block = push.first_block + int(group.y);
    int far_column = block * int(WAVE) + int(WAVE) - 1;
    line.far_u = column_u(s, line, far_column);
    float near_u = column_u(s, line, block * int(WAVE));
    float lowest;
    float highest;
    if (!rows(s, line.far_u, near_u, size, lowest, highest)) {
        return;
    }
    int first_row = int(floor(light_minor + lowest)) - ROW_MARGIN;
    int row = first_row + int(group.x);
    if (float(row) > light_minor + highest + 1.0) {
        return;
    }
    line.far_v = float(row) + 0.5 - light_minor;

    bool toward = push.march > 0.0;
    int first_sample = toward ? block * int(WAVE) - int(SAMPLES) : block * int(WAVE);
    int extent_major = int(s.major == 0u ? size.x : size.y);
    int extent_minor = int(s.major == 0u ? size.y : size.x);
    for (uint index = lane; index < SPAN; index += WAVE) {
        int column = first_sample + int(index);
        float depth = 0.0;
        int along = int(line.base) + int(s.side) * column;
        if (column >= 0 && along >= 0 && along < extent_major) {
            float u = column_u(s, line, column);
            float minor = light_minor + line.far_v * u / line.far_u - 0.5;
            int low = int(floor(minor));
            float t = minor - float(low);
            int high = low + 1;
            float2 corner = float2(pixel_of(s, along, low)) + 1.0;
            float4 gathered = depth_image.GatherRed(rikka_samplers[NEAREST_CLAMP], corner / view.capacity);
            float below = low >= 0 && low < extent_minor ? gathered.w : 0.0;
            float above = high >= 0 && high < extent_minor
                ? (s.major == 0u ? gathered.x : gathered.z)
                : 0.0;
            bool smooth = below > 0.0 && above > 0.0
                && abs(below - above) <= BLEND_LIMIT * max(below, above);
            depth = smooth ? lerp(below, above, t) : -min(below, above);
        }
        depths[index] = depth;
    }
    int column = block * int(WAVE) + int(lane);
    float v = line.far_v * column_u(s, line, column) / line.far_u;
    int along = int(line.base) + int(s.side) * column;
    int64_t light_major_fixed = to_fixed(light_major);
    Fan fan;
    fan.light_minor = to_fixed(light_minor);
    fan.u = fixed_u(s, light_major_fixed, along);
    fan.far_u = fixed_u(s, light_major_fixed, int(line.base) + int(s.side) * far_column);
    int across = crossing(fan, row);
    int first = row == first_row
        ? sector_floor(light_major_fixed, fan.light_minor, along)
        : crossing(fan, row - 1) + 1;
    first = max(first, 0);
    int last = min(across, extent_minor - 1);
    GroupMemoryBarrierWithGroupSync();
    if (along < 0 || along >= extent_major) {
        return;
    }
    uint own = toward ? SAMPLES + lane : lane;
    for (int minor = first; minor <= last; ++minor) {
        int2 pixel = pixel_of(s, along, minor);
        if (!in_sector(s, pixel)) {
            continue;
        }
        Receiver receiver;
        receiver.column = column;
        receiver.along = along;
        receiver.minor = minor;
        receiver.v = v;
        receiver.raw = depth_image.Load(int3(pixel, 0)).r;
        receiver.normal = rikka_read_surface(frame.look.scene.surface, pixel).normal;
        receiver.line_depth = minor == across ? depths[own] : 0.0;
        receiver.own = own;
        rikka_storage_r32f[push.target][pixel] = march(s, line, size, receiver);
    }
}
