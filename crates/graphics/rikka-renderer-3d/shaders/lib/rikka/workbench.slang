__exported import rikka.material;

static const uint RIKKA_VERTEX_COLOR_OFF = 0u;
static const uint RIKKA_VERTEX_COLOR_SRGB = 1u;

static const uint RIKKA_WORKBENCH_MATERIAL = 0u;
static const uint RIKKA_WORKBENCH_TEXTURE = 1u;
static const uint RIKKA_WORKBENCH_RANDOM = 2u;

static const uint RIKKA_WORKBENCH_STUDIO = 0u;
static const uint RIKKA_WORKBENCH_MATCAP = 1u;
static const uint RIKKA_WORKBENCH_SHADOWED = 0u;

static const float RIKKA_WORKBENCH_RANDOM_SATURATION = 0.5;
static const float RIKKA_WORKBENCH_RANDOM_VALUE = 0.8;
static const float RIKKA_MATCAP_SCALE = 0.496;
static const float3 RIKKA_STUDIO_AMBIENT = float3(0.0);

static const float3 RIKKA_STUDIO_DIRECTIONS[4] = {
    float3(-0.352546, 0.170931, -0.920051),
    float3(-0.408163, 0.346939, 0.844415),
    float3(0.521739, 0.826087, 0.212999),
    float3(0.624519, -0.562067, -0.542269),
};

static const float3 RIKKA_STUDIO_DIFFUSE[4] = {
    float3(0.033103, 0.033103, 0.033103),
    float3(0.521083, 0.538226, 0.538226),
    float3(0.038403, 0.034357, 0.049530),
    float3(0.090838, 0.082080, 0.072255),
};

static const float3 RIKKA_STUDIO_SPECULAR[4] = {
    float3(0.266761, 0.266761, 0.266761),
    float3(0.599030, 0.599030, 0.599030),
    float3(0.106102, 0.125981, 0.158523),
    float3(0.106535, 0.084771, 0.066080),
};

static const float4 RIKKA_STUDIO_WRAP = float4(0.526620, 0.0, 0.478261, 0.2);

struct RikkaStudio {
    float3 ambient;
    float3 directions[4];
    float3 diffuse[4];
    float3 specular[4];
    float4 wrap;
};

RikkaStudio rikka_studio(RikkaFrameRef frame) {
    RikkaStudio studio;
    if (frame.editor != 0ul && rikka_editor(frame).studio != 0u) {
        RikkaEditorRef editor = rikka_editor(frame);
        studio.ambient = editor.ambient.xyz;
        for (uint k = 0u; k < RIKKA_STUDIO_LIGHTS; k++) {
            RikkaStudioLight light = editor.lights[k];
            studio.directions[k] = light.direction.xyz;
            studio.diffuse[k] = light.diffuse.xyz;
            studio.specular[k] = light.specular.xyz;
            studio.wrap[k] = light.diffuse.w;
        }
        return studio;
    }
    studio.ambient = RIKKA_STUDIO_AMBIENT;
    for (uint k = 0u; k < RIKKA_STUDIO_LIGHTS; k++) {
        studio.directions[k] = RIKKA_STUDIO_DIRECTIONS[k];
        studio.diffuse[k] = RIKKA_STUDIO_DIFFUSE[k];
        studio.specular[k] = RIKKA_STUDIO_SPECULAR[k];
    }
    studio.wrap = RIKKA_STUDIO_WRAP;
    return studio;
}

float4 rikka_vertex_color(float4 color, uint mode) {
    if (mode == RIKKA_VERTEX_COLOR_OFF) {
        return float4(1.0);
    }
    if (mode == RIKKA_VERTEX_COLOR_SRGB) {
        return float4(rikka_decode_srgb(color.rgb), color.a);
    }
    return color;
}

float3 rikka_hsv_to_rgb(float3 hsv) {
    float3 k = rikka_mod(float3(5.0, 3.0, 1.0) + hsv.x * 6.0, float3(6.0));
    float3 ramp = clamp(min(k, 4.0 - k), 0.0, 1.0);
    return hsv.z - hsv.z * hsv.y * ramp;
}

float4 rikka_wrapped_lighting(float4 n_dot_l, float4 wrap) {
    float4 grown = wrap + 1.0;
    return clamp((n_dot_l + wrap) / (grown * grown), 0.0, 1.0);
}

float3 rikka_studio_lighting(RikkaStudio studio, float3 base_color, float roughness, float metallic, float3 n, float3 incident, bool specular) {
    float3 diffuse_color = base_color;
    float3 specular_color = float3(0.0);
    if (specular) {
        diffuse_color = lerp(base_color, float3(0.0), metallic);
        specular_color = lerp(float3(0.05), base_color, metallic);
    }
    float3 specular_light = studio.ambient;
    float3 diffuse_light = studio.ambient;
    float4 wrap = studio.wrap;

    if (specular) {
        float3 mirrored = -reflect(incident, n);
        float4 spec_angle;
        float4 spec_n_dot_l;
        float4 wrap_n_dot_l;
        for (uint k = 0u; k < 4u; k++) {
            float3 l = studio.directions[k];
            wrap_n_dot_l[k] = dot(l, mirrored);
            spec_angle[k] = clamp(dot(normalize(l + incident), n), 0.0, 1.0);
            spec_n_dot_l[k] = clamp(dot(l, n), 0.0, 1.0);
        }
        float4 gloss = float4(1.0 - roughness) * (1.0 - wrap);
        float4 shininess = exp2(10.0 * gloss + 1.0);
        float4 spec_light = pow(spec_angle, shininess) * spec_n_dot_l * (shininess * 0.125 + 1.0);
        float4 environment = rikka_wrapped_lighting(wrap_n_dot_l, lerp(wrap, float4(1.0), roughness));
        spec_light = lerp(spec_light, environment, wrap * wrap);
        for (uint k = 0u; k < 4u; k++) {
            specular_light += spec_light[k] * studio.specular[k];
        }
        float n_dot_v = clamp(dot(n, incident), 0.0, 1.0);
        float fresnel = exp2(-8.35 * n_dot_v) * (1.0 - roughness);
        specular_color = lerp(specular_color, float3(1.0), fresnel);
    }
    specular_light *= specular_color;

    float4 diffuse_n_dot_l;
    for (uint k = 0u; k < 4u; k++) {
        diffuse_n_dot_l[k] = dot(studio.directions[k], n);
    }
    float4 diffuse = rikka_wrapped_lighting(diffuse_n_dot_l, wrap);
    for (uint k = 0u; k < 4u; k++) {
        diffuse_light += diffuse[k] * studio.diffuse[k];
    }
    float specular_energy = dot(specular_color, float3(1.0 / 3.0));
    diffuse_light *= diffuse_color * (1.0 - specular_energy);
    return diffuse_light + specular_light;
}

float rikka_workbench_shadow(RikkaPixel pixel, RikkaContext context, float3 n) {
    RikkaFrameRef frame = context.frame;
    if (frame.editor == 0ul || context.view.directional_count == 0u) {
        return 1.0;
    }
    RikkaEditorRef editor = rikka_editor(frame);
    if (editor.shadowed == 0u) {
        return 1.0;
    }
    RikkaShadowPoint point;
    point.world_position = pixel.world_position;
    point.geometric_normal = pixel.geometric_normal;
    point.view_depth = pixel.view_depth;
    point.flags = context.instance.flags | RIKKA_RECEIVES_SHADOW;
    point.rotation = rikka_shadow_rotation(frame, pixel.frag_coord.xy);
    point.traced = pixel.traced;
    float visibility = rikka_shadow_evaluate(frame, frame.lights[0], point).visibility;
    float3 direction = normalize(mul(context.view.view, float4(editor.shadow.xyz, 0.0)).xyz);
    float light_factor = -dot(n, direction);
    float shadow_mix = smoothstep(editor.shadow_shift, editor.shadow_focus, light_factor);
    float strength = editor.shadow.w;
    return shadow_mix * visibility * strength + (1.0 - strength);
}

float2 rikka_matcap_uv(float3 incident, float3 n, bool flipped) {
    float a = 1.0 / (1.0 + incident.z);
    float b = -incident.x * incident.y * a;
    float3 b1 = float3(1.0 - incident.x * incident.x * a, b, -incident.x);
    float3 b2 = float3(b, 1.0 - incident.y * incident.y * a, -incident.y);
    float2 uv = float2(dot(b1, n), dot(b2, n));
    if (flipped) {
        uv.x = -uv.x;
    }
    return uv * RIKKA_MATCAP_SCALE + 0.5;
}

struct RikkaWorkbenchData {
    float metallic;
    float roughness;
};

struct RikkaWorkbenchSurface : ISurface<RikkaWorkbenchData> {
    [Options("material|texture|random")] [Default(0)] [Label("Color")] uint color_source;
    [Color] [Srgb] [Default(0.8, 0.8, 0.8)] [Role("albedo")] [Label("Base color")] float3 base_color;
    [Texture("colour")] [Role("albedo_texture")] RikkaTexture base_color_texture;
    [Options("off|srgb|linear")] [Default(0)] [Label("Vertex color")] uint vertex_color;
    [Default(0.0)] [Range(0.0, 1.0)] float metallic;
    [Default(0.4)] [Range(0.0, 1.0)] float roughness;
    [Default(1.0)] [Range(0.0, 1.0)] float alpha;

    [constref]
    RikkaSurfaceData<RikkaWorkbenchData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaWorkbenchData data;
        data.metallic = metallic;
        data.roughness = roughness;
        RikkaSurfaceData<RikkaWorkbenchData> s = rikka_surface_data(pixel, data);
        float4 color = float4(base_color, 1.0);
        if (color_source == RIKKA_WORKBENCH_TEXTURE) {
            color = base_color_texture.sampled(pixel.uv);
        } else if (color_source == RIKKA_WORKBENCH_RANDOM) {
            float hue = rikka_unit(rikka_hash(context.instance.object_id));
            color = float4(rikka_hsv_to_rgb(float3(hue, RIKKA_WORKBENCH_RANDOM_SATURATION, RIKKA_WORKBENCH_RANDOM_VALUE)), 1.0);
        }
        float4 painted = rikka_vertex_color(pixel.color, vertex_color);
        s.albedo = color.rgb * painted.rgb;
        s.alpha = alpha * color.a * painted.a;
        return s;
    }

    [constref]
    override float coverage(RikkaCoverage coverage, RikkaContext context) {
        if (color_source == RIKKA_WORKBENCH_TEXTURE) {
            return alpha * base_color_texture.sampled(coverage.uv).a;
        }
        return alpha;
    }
};

struct RikkaWorkbenchState {
    float3 albedo;
    float alpha;
    float3 normal;
    float metallic;
    float roughness;
};

struct RikkaWorkbenchModel : IShadingModel {
    typealias Data = RikkaWorkbenchData;
    typealias State = RikkaWorkbenchState;

    [Options("studio|matcap|flat")] [Default(0)] uint lighting;
    [Default(1.0)] bool specular;
    [Texture("colour")] RikkaTexture matcap;
    [Texture("colour")] RikkaTexture matcap_specular;
    [Default(0.0)] bool matcap_flip;
    [Default(0.0)] [Feature(0)] bool shadowed;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        s.albedo = surface.albedo;
        s.alpha = surface.alpha;
        s.normal = surface.normal;
        s.metallic = surface.model.metallic;
        s.roughness = surface.model.roughness;
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
        RikkaViewRef view = context.view;
        float3 n = normalize(mul(view.view, float4(state.normal, 0.0)).xyz);
        float3 position = mul(view.view, float4(pixel.world_position, 1.0)).xyz;
        bool perspective = view.projection[3][3] < 0.5;
        float3 incident = perspective ? normalize(-position) : float3(0.0, 0.0, 1.0);
        float3 color = state.albedo;
        if (lighting == RIKKA_WORKBENCH_STUDIO) {
            color = rikka_studio_lighting(rikka_studio(context.frame), state.albedo, state.roughness, state.metallic, n, incident, specular);
        } else if (lighting == RIKKA_WORKBENCH_MATCAP) {
            float2 uv = rikka_matcap_uv(incident, n, matcap_flip);
            color = matcap.sampled_level(uv, 0.0).rgb * state.albedo;
            if (specular && matcap_specular.bound()) {
                color += matcap_specular.sampled_level(uv, 0.0).rgb;
            }
        }
        if (rikka_feature(RIKKA_WORKBENCH_SHADOWED) && shadowed) {
            color *= rikka_workbench_shadow(pixel, context, n);
        }
        return float4(color, state.alpha);
    }

    [constref]
    override float4 shade(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        return resolve(prepare(pixel, surface, context), pixel, context);
    }
};

struct RikkaUnlitSurface : ISurface<RikkaWorkbenchData> {
    [Color] [Srgb] [Default(1.0, 1.0, 1.0)] [Role("albedo")] float3 color;
    [Texture("colour")] [Role("albedo_texture")] RikkaTexture color_texture;
    [Options("off|srgb|linear")] [Default(0)] [Label("Vertex color")] uint vertex_color;
    [Default(1.0)] [Range(0.0, 1.0)] float alpha;

    [constref]
    RikkaSurfaceData<RikkaWorkbenchData> evaluate(RikkaPixel pixel, RikkaContext context) {
        RikkaWorkbenchData data;
        data.metallic = 0.0;
        data.roughness = 1.0;
        RikkaSurfaceData<RikkaWorkbenchData> s = rikka_surface_data(pixel, data);
        float4 texel = color_texture.sampled(pixel.uv);
        float4 painted = rikka_vertex_color(pixel.color, vertex_color);
        s.albedo = color * texel.rgb * painted.rgb;
        s.alpha = alpha * texel.a * painted.a;
        return s;
    }

    [constref]
    override float coverage(RikkaCoverage coverage, RikkaContext context) {
        return alpha * color_texture.sampled(coverage.uv).a;
    }
};

struct RikkaShadelessModel : IShadingModel {
    typealias Data = RikkaWorkbenchData;
    typealias State = RikkaWorkbenchState;

    [constref]
    State prepare(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        State s;
        s.albedo = surface.albedo + surface.emission;
        s.alpha = surface.alpha;
        s.normal = surface.normal;
        s.metallic = surface.model.metallic;
        s.roughness = surface.model.roughness;
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
        return float4(state.albedo, state.alpha);
    }

    [constref]
    override float4 shade(RikkaPixel pixel, RikkaSurfaceData<Data> surface, RikkaContext context) {
        return resolve(prepare(pixel, surface, context), pixel, context);
    }
};
