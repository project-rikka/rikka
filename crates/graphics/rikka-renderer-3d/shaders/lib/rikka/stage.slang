__exported import rikka.standard;

struct RikkaNoData {
    uint unused;
};

struct RikkaPlainSurface : ISurface<RikkaNoData> {
    [constref]
    RikkaSurfaceData<RikkaNoData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaNoData data;
        data.unused = 0u;
        return rikka_surface_data(pixel, data);
    }
};

struct RikkaUnlitState {
    float3 color;
    float alpha;
};

struct RikkaUnlitModel : IShadingModel {
    typealias Data = RikkaNoData;
    typealias State = RikkaUnlitState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        s.color = surface.albedo + surface.emission;
        s.alpha = surface.alpha;
        return s;
    }

    [constref]
    void direct(inout State state, RikkaLightSample light, inout RikkaShadowQuery shadow, RikkaContext context) {
    }

    [constref]
    void indirect(inout State state, RikkaIndirect indirect, RikkaContext context) {
    }

    [constref]
    float4 resolve(State state, RikkaPixel pixel, RikkaContext context) {
        return float4(state.color, state.alpha);
    }
};

struct RikkaWireSurface : ISurface<RikkaNoData> {
    [Color] [Default(1.0, 1.0, 1.0)] float3 color;
    [Default(1.0)] [SoftRange(0.0, 1.0)] float alpha;
    [Default(0.0)] [SoftRange(0.0, 1.0)] float shading;

    [constref]
    RikkaSurfaceData<RikkaNoData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaNoData data;
        data.unused = 0u;
        RikkaSurfaceData<RikkaNoData> s = rikka_surface_data(pixel, data);
        float3 shaded = color;
        if (shading > 0.0) {
            float facing = dot(normalize(pixel.geometric_normal), pixel.view) * 0.5 + 0.5;
            shaded *= lerp(1.0, facing, shading);
        }
        s.albedo = shaded * alpha;
        s.alpha = alpha;
        return s;
    }
};

struct RikkaFloorPattern {
    float shade;
    float seam;
    float roughness;
};

float rikka_floor_hash(float2 cell) {
    uint2 q = uint2(int2(cell) + 32768);
    uint h = q.x * 1597334677u ^ q.y * 3812015801u;
    h = (h ^ (h >> 16u)) * 2246822519u;
    h ^= h >> 13u;
    return rikka_unit(h);
}

float rikka_floor_noise(float2 p) {
    float2 cell = floor(p);
    float2 f = fract(p);
    float2 u = f * f * (3.0 - 2.0 * f);
    float a = rikka_floor_hash(cell);
    float b = rikka_floor_hash(cell + float2(1.0, 0.0));
    float c = rikka_floor_hash(cell + float2(0.0, 1.0));
    float d = rikka_floor_hash(cell + float2(1.0, 1.0));
    return lerp(lerp(a, b, u.x), lerp(c, d, u.x), u.y);
}

float rikka_floor_seam(float edge_distance, float seam) {
    float soft = max(fwidth(edge_distance), 1e-5);
    return 1.0 - smoothstep(seam * 0.5 - soft, seam * 0.5 + soft, edge_distance);
}

RikkaFloorPattern rikka_floor_tiles(float2 plane, float scale, float seam) {
    float2 cell = floor(plane);
    float2 f = fract(plane);
    float2 edges = min(f, 1.0 - f) * scale;
    float variation = rikka_floor_hash(cell);
    RikkaFloorPattern p;
    p.shade = variation - 0.5;
    p.seam = rikka_floor_seam(min(edges.x, edges.y), seam);
    p.roughness = rikka_floor_hash(cell + float2(17.0, 5.0)) - 0.5;
    return p;
}

RikkaFloorPattern rikka_floor_planks(float2 plane, float scale, float seam) {
    const float LENGTH = 4.0;
    float row = floor(plane.y);
    float shift = rikka_floor_hash(float2(row, 3.0)) * LENGTH;
    float along = (plane.x + shift) / LENGTH;
    float2 cell = float2(floor(along), row);
    float2 edges = float2(min(fract(along), 1.0 - fract(along)) * LENGTH, min(fract(plane.y), 1.0 - fract(plane.y))) * scale;
    float variation = rikka_floor_hash(cell);
    float grain = rikka_floor_noise(float2(plane.x * 1.5 + cell.x * 7.0, fract(plane.y) * 24.0 + variation * 9.0));
    float rings = 0.5 + 0.5 * sin((fract(plane.y) * 18.0 + grain * 5.0 + variation * 20.0));
    RikkaFloorPattern p;
    p.shade = (variation - 0.5) + (rings - 0.5) * 0.35;
    p.seam = rikka_floor_seam(min(edges.x, edges.y), seam);
    p.roughness = (grain - 0.5) * 0.6;
    return p;
}

static const uint RIKKA_FLOOR_PLAIN = 0u;
static const uint RIKKA_FLOOR_TILES = 1u;
static const uint RIKKA_FLOOR_PLANKS = 2u;

struct RikkaFloorSurface : ISurface<RikkaStandardData> {
    [Color] [Srgb] [Default(0.42, 0.4, 0.42)] [Role("albedo")] float3 color;
    [Default(0.18)] [SoftRange(0.02, 1.0)] float roughness;
    [Default(0.0)] [SoftRange(0.0, 1.0)] float metallic;
    [Options("plain|tiles|planks")] [Default(0)] uint pattern;
    [Default(1.0)] [SoftRange(0.05, 10.0)] float scale;
    [Default(0.25)] [SoftRange(0.0, 1.0)] float contrast;
    [Default(0.012)] [SoftRange(0.0, 0.1)] float seam;
    [Color] [Srgb] [Default(0.08, 0.08, 0.09)] float3 seam_color;
    [Default(1.0, 1.0)] float2 texture_scale;
    [Texture("colour")] RikkaTexture texture;
    [Texture("normal")] RikkaTexture normal_texture;

    [constref]
    RikkaSurfaceData<RikkaStandardData> evaluate(RikkaPixel pixel, RikkaContext context) {
        float2 uv = pixel.uv * texture_scale;
        float4 texel = texture.sampled(uv);
        float3 albedo = color * pixel.color.rgb * texel.rgb;
        float rough = roughness;

        float size = max(scale, 1e-3);
        if (pattern != RIKKA_FLOOR_PLAIN) {
            float2 plane = pixel.world_position.xz / size;
            RikkaFloorPattern p = pattern == RIKKA_FLOOR_PLANKS
                ? rikka_floor_planks(plane, size, seam)
                : rikka_floor_tiles(plane, size, seam);
            float2 footprint = fwidth(plane);
            float detail = 1.0 - smoothstep(0.25, 1.0, max(footprint.x, footprint.y));
            float strength = contrast * detail;
            albedo *= max(1.0 + p.shade * strength, 0.0);
            rough = clamp(rough * (1.0 + p.roughness * strength), 0.02, 1.0);
            float seamed = p.seam * detail;
            albedo = lerp(albedo, seam_color, seamed);
            rough = lerp(rough, max(rough, 0.8), seamed);
        }

        RikkaStandardData model;
        model.metallic = metallic;
        model.roughness = rough;
        RikkaSurfaceData<RikkaStandardData> s = rikka_surface_data(pixel, model);
        if (normal_texture.bound()) {
            s.normal = pixel.normal_map(normalize(rikka_unpack_normal(normal_texture.sampled(uv))));
        }
        s.albedo = albedo;
        s.alpha = texel.a;
        return s;
    }

    [constref]
    override float coverage(RikkaCoverage coverage, RikkaContext context) {
        return texture.bound() ? texture.sampled(coverage.uv).a : 1.0;
    }
};

struct RikkaShadowCatcherState {
    float lit;
};

struct RikkaShadowCatcherModel : IShadingModel {
    [Color] [Srgb] [Default(0.0, 0.0, 0.0)] float3 color;
    [Default(0.5)] [SoftRange(0.0, 1.0)] float shadow;

    typealias Data = RikkaNoData;
    typealias State = RikkaShadowCatcherState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        s.lit = 1.0;
        return s;
    }

    [constref]
    void direct(inout State s, RikkaLightSample light, inout RikkaShadowQuery query, RikkaContext context) {
        if (light.light.shadow == RIKKA_NONE || light.light.shadow_count == 0u) {
            return;
        }
        s.lit = min(s.lit, query.map());
    }

    [constref]
    void indirect(inout State s, RikkaIndirect indirect, RikkaContext context) {
    }

    [constref]
    float4 resolve(State s, RikkaPixel pixel, RikkaContext context) {
        return float4(color, (1.0 - s.lit) * shadow);
    }
};

static const uint RIKKA_WATER_OCTAVES = 9u;
static const float RIKKA_WATER_GRAVITY = 9.81;
static const float RIKKA_WATER_GLINT_CELLS = 0.045;

struct RikkaWaves {
    float2 slope;
    float crest;
};

float rikka_water_hash(float3 p) {
    uint3 q = uint3(int3(p) + 65536);
    uint h = q.x * 1597334677u ^ q.y * 3812015801u ^ q.z * 2798796415u;
    h = (h ^ (h >> 16u)) * 2246822519u;
    h ^= h >> 13u;
    return rikka_unit(h);
}

RikkaWaves rikka_waves(float2 at, float time, float2 wind, float scale, float speed, float footprint) {
    RikkaWaves w;
    w.slope = float2(0.0);
    w.crest = 0.0;
    float wavelength = scale;
    float total = 0.0;
    float turn = 0.0;
    float2 warp = float2(
        sin(dot(at, float2(0.13, 0.71)) * 2.1 / scale + time * 0.11),
        cos(dot(at, float2(0.83, -0.27)) * 1.7 / scale - time * 0.13)) * scale * 0.3;
    for (uint octave = 0u; octave < RIKKA_WATER_OCTAVES; ++octave) {
        float fade = smoothstep(2.0 * footprint, 6.0 * footprint, wavelength);
        if (fade <= 0.0) {
            break;
        }
        float spread = octave == 0u ? 0.0 : 0.5;
        float angle = sin(turn) * spread * 1.4;
        float2 direction = float2(wind.x * cos(angle) - wind.y * sin(angle), wind.x * sin(angle) + wind.y * cos(angle));
        float2 place = at + warp * float(min(octave, 2u)) * 0.5;
        float k = 2.0 * RIKKA_PI / wavelength;
        float omega = sqrt(RIKKA_WATER_GRAVITY * k) * speed;
        float amplitude = wavelength * 0.03 * pow(0.92, float(octave));
        float phase = k * dot(direction, place) - omega * time + float(octave) * 1.7;
        w.slope += direction * (amplitude * k * cos(phase) * fade);
        w.crest += amplitude * sin(phase) * fade;
        total += amplitude;
        wavelength *= 0.63;
        turn += 2.39996;
    }
    w.crest = total > 0.0 ? w.crest / total : 0.0;
    return w;
}

float rikka_water_glints(float2 at, float time, float3 normal, float3 eye, float3 toward, float footprint) {
    float cell_size = max(RIKKA_WATER_GLINT_CELLS, footprint * 1.5);
    float2 cell = floor(at / cell_size);
    float clock = time * 3.0 + rikka_water_hash(float3(cell, 7.0)) * 8.0;
    float3 seed = float3(cell, floor(clock));
    float2 centre = (cell + 0.25 + 0.5 * float2(rikka_water_hash(seed), rikka_water_hash(seed + 11.0))) * cell_size;
    float3 tilt = float3(rikka_water_hash(seed + 23.0), 0.0, rikka_water_hash(seed + 37.0)) * 2.0 - 1.0;
    float3 facet = normalize(normal + tilt * 0.12);
    float alignment = max(dot(reflect(-eye, facet), toward), 0.0);
    float spot = 1.0 - smoothstep(0.0, cell_size * 0.22, length(at - centre));
    float life = sin(fract(clock) * RIKKA_PI);
    return pow(alignment, 400.0) * spot * life;
}

struct RikkaWaterState {
    RikkaStandardState standard;
    float3 flat_normal;
    float3 normal;
    float3 view;
    float3 toward;
    float3 sun;
    float shadow;
    float2 at;
    float footprint;
    float time;
};

struct RikkaWaterModel : IShadingModel {
    [Color] [Default(0.004, 0.018, 0.03)] float3 deep;
    [Default(1.0)] [SoftRange(0.0, 4.0)] float height;
    [Color] [Default(0.03, 0.16, 0.2)] float3 shallow;
    [Default(4.0)] [SoftRange(0.1, 20.0)] float scale;
    [Default(1.0)] [SoftRange(0.0, 4.0)] float speed;
    [Default(1.0)] [SoftRange(0.0, 4.0)] float glint;
    [Default(1.0, 0.35)] float2 direction;
    [Default(0.05)] [SoftRange(0.02, 1.0)] float roughness;
    RikkaGloss gloss;

    typealias Data = RikkaNoData;
    typealias State = RikkaWaterState;

    [constref]
    RikkaStandardModel standard() {
        RikkaStandardModel model;
        model.gloss = gloss;
        return model;
    }

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        RikkaFrameRef frame = context.frame;
        RikkaViewRef view = context.view;
        s.time = view.time;
        float size = max(scale, 1e-3);
        float2 wind = dot(direction, direction) > 1e-8 ? normalize(direction) : float2(1.0, 0.0);

        s.at = pixel.world_position.xz;
        float2 footprint2 = fwidth(s.at);
        s.footprint = max(max(footprint2.x, footprint2.y), 1e-4);
        RikkaWaves w = rikka_waves(s.at, s.time, wind, size, speed, s.footprint);
        s.flat_normal = pixel.geometric_normal;
        s.normal = normalize(s.flat_normal + float3(-w.slope.x, 0.0, -w.slope.y) * height);
        s.view = pixel.view;

        s.toward = float3(0.0, 1.0, 0.0);
        s.sun = float3(0.0);
        s.shadow = 1.0;
        if (view.directional_count > 0u) {
            RikkaLight light = frame.lights[0];
            s.toward = -light.direction;
            s.sun = light.color * light.intensity;
            RikkaShadowPoint point;
            point.world_position = pixel.world_position;
            point.geometric_normal = pixel.geometric_normal;
            point.view_depth = pixel.view_depth;
            point.flags = context.instance.flags;
            point.rotation = rikka_shadow_rotation(frame, pixel.frag_coord.xy);
            point.traced = pixel.traced;
            s.shadow = rikka_shadow(frame, light, point);
        }
        float behind_light = pow(max(dot(s.view, -s.toward), 0.0), 4.0);
        float lift = clamp(0.35 * max(w.crest, 0.0) + 0.5 * behind_light * max(w.crest + 0.3, 0.0), 0.0, 1.0);

        RikkaStandardData data;
        data.metallic = 0.0;
        data.roughness = roughness;
        RikkaSurfaceData<RikkaStandardData> water = rikka_surface_data(pixel, data);
        water.albedo = lerp(deep, shallow, lift);
        water.normal = s.normal;
        s.standard = standard().prepare(pixel, water, context);
        return s;
    }

    [constref]
    void direct(inout State s, RikkaLightSample light, inout RikkaShadowQuery query, RikkaContext context) {
        standard().direct(s.standard, light, query, context);
    }

    [constref]
    void indirect(inout State s, RikkaIndirect indirect, RikkaContext context) {
        standard().indirect(s.standard, indirect, context);
    }

    [constref]
    float4 resolve(State s, RikkaPixel pixel, RikkaContext context) {
        float3 color = s.standard.lit;
        float reflectance = rikka_reflectance(gloss.reflection, gloss.roughness, max(dot(s.flat_normal, s.view), 1e-4));
        float3 rippled = rikka_sky_specular(context.frame, reflect(-s.view, s.normal), gloss.roughness);
        float3 still = rikka_sky_specular(context.frame, reflect(-s.view, s.flat_normal), gloss.roughness);
        color = max(color + (rippled - still) * reflectance, float3(0.0));
        float sparkle = rikka_water_glints(s.at, s.time, s.normal, s.view, s.toward, s.footprint);
        color += s.sun * s.shadow * sparkle * glint * 40.0;
        return float4(color, rikka_coverage(context, 1.0));
    }

    [constref]
    override RikkaGBuffer gbuffer(RikkaContext context) {
        return rikka_gbuffer(context, gloss);
    }
};
