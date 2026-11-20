__exported import rikka.sky.atmosphere;
__exported import rikka.sky;

static const int RIKKA_FOG_SLICES = 64;
static const uint RIKKA_FOG_COLUMNS = 8u;
static const int RIKKA_AERIAL_SLICES = 32;
static const uint RIKKA_AERIAL_COLUMNS = 8u;
static const uint RIKKA_FOG_LIGHTS = 16u;

static const float RIKKA_FOG_NEAR = 1.0;
static const float RIKKA_FOG_FAR = 256.0;
static const float RIKKA_FOG_SKY_DISTANCE = 2000.0;

float rikka_fog_slice_depth(int edge) {
    if (edge <= 0) {
        return 0.0;
    }
    return RIKKA_FOG_NEAR * pow(RIKKA_FOG_FAR / RIKKA_FOG_NEAR,
                                float(edge) / float(RIKKA_FOG_SLICES));
}

float rikka_fog_density_at(RikkaSky sky, float height) {
    return sky.fog_density * exp(-max(height, 0.0) / sky.fog_height);
}

float3 rikka_fog_ambient(RikkaFrameRef frame, RikkaSky sky, float3 ray) {
    float3 above = rikka_sky_irradiance(frame, float3(0.0, 1.0, 0.0));
    float3 horizon = normalize(float3(ray.x, 0.04, ray.z) + float3(1e-4, 0.0, 0.0));
    if (sky.physical != 0u) {
        return 0.5 * (above + rikka_sky_view(sky, horizon).rgb);
    }
    if (sky.environment != 0u) {
        return 0.5 * (above + rikka_sky_specular(frame, horizon, 1.0));
    }
    return above;
}

float rikka_fog_phase(RikkaSky sky, float c) {
    return rikka_henyey_greenstein(sky.fog_forward, c);
}

float3 rikka_fog_daylight(RikkaFrameRef frame, RikkaSky sky, float3 ray) {
    RikkaViewRef view = frame.view;
    float3 light = float3(0.0);
    uint count = min(view.directional_count, RIKKA_FOG_LIGHTS);
    for (uint i = 0u; i < count; ++i) {
        RikkaLight item = frame.lights[i];
        if (item.figure != 0u) {
            continue;
        }
        light += item.color * item.intensity * rikka_fog_phase(sky, dot(ray, -item.direction));
    }
    return light * sky.fog_shafts;
}

struct RikkaShaftSource {
    float3 direction;
    float3 color;
};

RikkaShaftSource rikka_shaft_source(RikkaSky sky) {
    RikkaShaftSource source;
    bool sun = rikka_luma(sky.sun_color) >= rikka_luma(sky.moon_color);
    source.direction = sun ? sky.to_sun : sky.to_moon;
    source.color = sun ? sky.sun_color : sky.moon_color;
    return source;
}

float4 rikka_tiled_slice(uint image, uint2 grid, uint columns, float2 uv, int slice) {
    float2 cells = float2(grid);
    float2 tile = float2(float(uint(slice) % columns), float(uint(slice) / columns)) * cells;
    float2 inside = clamp(uv * cells, float2(0.5), cells - 0.5);
    float2 size = float2(rikka_image_size(rikka_images[image], 0));
    return rikka_images[image].SampleLevel(rikka_samplers[RIKKA_ATMO_SAMPLER_CLAMP],
                                           (tile + inside) / size, 0.0);
}

int2 rikka_tiled_texel(uint2 grid, uint columns, uint2 cell, int slice) {
    uint2 tile = uint2(uint(slice) % columns, uint(slice) / columns);
    return int2(tile * grid + cell);
}

float4 rikka_fog_volume_at(RikkaSky sky, float2 uv, float view_depth) {
    uint2 grid = sky.fog_grid.xy;
    float first = rikka_fog_slice_depth(1);
    if (view_depth <= first) {
        return lerp(float4(0.0, 0.0, 0.0, 1.0),
                    rikka_tiled_slice(sky.fog_volume, grid, RIKKA_FOG_COLUMNS, uv, 0),
                    view_depth / first);
    }
    float position = log(view_depth / RIKKA_FOG_NEAR) / log(RIKKA_FOG_FAR / RIKKA_FOG_NEAR)
        * float(RIKKA_FOG_SLICES) - 1.0;
    position = clamp(position, 0.0, float(RIKKA_FOG_SLICES - 1));
    int lower = int(floor(position));
    int upper = min(lower + 1, RIKKA_FOG_SLICES - 1);
    return lerp(rikka_tiled_slice(sky.fog_volume, grid, RIKKA_FOG_COLUMNS, uv, lower),
                rikka_tiled_slice(sky.fog_volume, grid, RIKKA_FOG_COLUMNS, uv, upper),
                position - float(lower));
}

float rikka_aerial_distance(RikkaSky sky, int slice) {
    float w = float(slice + 1) / float(RIKKA_AERIAL_SLICES);
    return sky.aerial_range * w * w;
}

float4 rikka_aerial_at(RikkaSky sky, float2 uv, float kilometres) {
    uint2 grid = rikka_image_size(rikka_images[sky.aerial_lut], 0)
        / uint2(RIKKA_AERIAL_COLUMNS, uint(RIKKA_AERIAL_SLICES) / RIKKA_AERIAL_COLUMNS);
    float position = sqrt(max(kilometres, 0.0) / sky.aerial_range) * float(RIKKA_AERIAL_SLICES)
        - 1.0;
    if (position <= 0.0) {
        return lerp(float4(0.0, 0.0, 0.0, 1.0),
                    rikka_tiled_slice(sky.aerial_lut, grid, RIKKA_AERIAL_COLUMNS, uv, 0),
                    position + 1.0);
    }
    position = min(position, float(RIKKA_AERIAL_SLICES - 1));
    int lower = int(floor(position));
    int upper = min(lower + 1, RIKKA_AERIAL_SLICES - 1);
    return lerp(rikka_tiled_slice(sky.aerial_lut, grid, RIKKA_AERIAL_COLUMNS, uv, lower),
                rikka_tiled_slice(sky.aerial_lut, grid, RIKKA_AERIAL_COLUMNS, uv, upper),
                position - float(lower));
}

float4 rikka_height_fog(RikkaFrameRef frame, RikkaSky sky, float3 start, float3 ray, float span) {
    float height = max(start.y, 0.0);
    float base = rikka_fog_density_at(sky, height);
    float climb = ray.y / sky.fog_height;
    float depth = abs(climb) > 1e-4
        ? base * (1.0 - exp(-span * climb)) / climb
        : base * span;
    float passed = exp(-depth);
    float3 light = rikka_fog_ambient(frame, sky, ray) + rikka_fog_daylight(frame, sky, ray);
    return float4(light * sky.fog_color * (1.0 - passed), passed);
}
