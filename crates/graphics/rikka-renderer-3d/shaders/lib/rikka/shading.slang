__exported import rikka.common;
__exported import rikka.sky;
__exported import rikka.space;
__exported import rikka.shadow;
__exported import rikka.light;
__exported import rikka.traced;

struct RikkaLightIter {
    uint sun;
    uint suns;
    uint near;
    uint near_end;
    uint figure;
    uint channels;
    uint key;
};

RikkaLightIter rikka_lights_begin(RikkaFrameRef frame, float2 frag_xy, float view_depth, uint figure, uint channels) {
    RikkaViewRef view = frame.view;
    RikkaLightIter it;
    it.sun = 0u;
    it.suns = view.directional_count;
    it.near = 0u;
    it.near_end = 0u;
    it.figure = figure;
    it.channels = channels;
    it.key = figure != 0u ? frame.figures[figure].key_light : RIKKA_NONE;

    if (view.cluster_stride != 0u) {
        uint cluster = rikka_cluster_index(view, frag_xy, view_depth);
        it.near = cluster * view.cluster_stride;
        it.near_end = it.near + frame.clusters[cluster].count;
    }
    return it;
}

RikkaLightIter rikka_lights_begin(RikkaFrameRef frame, float2 frag_xy, float view_depth) {
    return rikka_lights_begin(frame, frag_xy, view_depth, 0u, RIKKA_ALL_CHANNELS);
}

bool rikka_light_reaches(RikkaLightIter it, RikkaLight light) {
    return (light.channels & it.channels) != 0u && (light.figure == 0u || light.figure == it.figure);
}

bool rikka_light_next(RikkaFrameRef frame, inout RikkaLightIter it, out RikkaLight light) {
    if (it.sun < it.suns) {
        uint index = it.sun == 0u && it.key != RIKKA_NONE ? it.key : it.sun;
        light = frame.lights[index];
        it.sun += 1u;
        return true;
    }
    if (it.near < it.near_end) {
        light = frame.lights[frame.cluster_indices[it.near]];
        it.near += 1u;
        return true;
    }
    light = {};
    return false;
}

void rikka_light_sample(RikkaFrameRef frame, RikkaLight light, float3 world_position, out float3 direction, out float attenuation) {
    RikkaLightSample s = rikka_sample_light(frame, light, world_position);
    direction = s.direction;
    attenuation = s.attenuation;
}

float rikka_ggx_distribution(float n_dot_h, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / max(3.14159265 * d * d, 1e-7);
}

float rikka_smith_visibility(float n_dot_v, float n_dot_l, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float v = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - a2) + a2);
    float l = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - a2) + a2);
    return 0.5 / max(v + l, 1e-7);
}

float3 rikka_fresnel(float3 f0, float v_dot_h) {
    float f = pow(clamp(1.0 - v_dot_h, 0.0, 1.0), 5.0);
    return f0 + (float3(1.0) - f0) * f;
}

float2 rikka_environment_brdf(float n_dot_v, float roughness) {
    const float4 c0 = float4(-1.0, -0.0275, -0.572, 0.022);
    const float4 c1 = float4(1.0, 0.0425, 1.04, -0.04);
    float4 r = roughness * c0 + c1;
    float a004 = min(r.x * r.x, exp2(-9.28 * n_dot_v)) * r.x + r.y;
    return float2(-1.04, 1.04) * a004 + r.zw;
}

float rikka_specular_occlusion(float n_dot_v, float occlusion, float roughness) {
    return clamp(pow(n_dot_v + occlusion, exp2(-16.0 * roughness - 1.0)) - 1.0 + occlusion, 0.0, 1.0);
}

static const float RIKKA_LOG2_TEN = 3.321928;

float rikka_cap_overlap(float cos_a, float cos_b, float cos_between) {
    float a = acos(cos_a);
    float b = acos(cos_b);
    float between = acos(clamp(cos_between, -1.0, 1.0));
    float cap = 1.0 - max(cos_a, cos_b);
    if (min(a, b) <= max(a, b) - between) {
        return cap;
    }
    if (a + b <= between) {
        return 0.0;
    }
    float spread = abs(a - b);
    float x = 1.0 - clamp((between - spread) / max(a + b - spread, 1e-4), 0.0, 1.0);
    return x * x * (3.0 - 2.0 * x) * cap;
}

float rikka_cone_occlusion(float3 bent, float visibility, float3 normal, float3 reflected, float roughness) {
    float alpha = roughness * roughness;
    float cos_lobe = exp2(-RIKKA_LOG2_TEN * alpha * alpha);
    float cos_visible = sqrt(clamp(1.0 - visibility, 0.0, 1.0));
    float open = rikka_cap_overlap(0.0, cos_lobe, dot(normal, reflected));
    if (open <= 0.0) {
        return 1.0;
    }
    return clamp(rikka_cap_overlap(cos_visible, cos_lobe, dot(bent, reflected)) / open, 0.0, 1.0);
}

float rikka_bent_specular_occlusion(float3 bent, float visibility, float3 normal, float3 view, float roughness) {
    float n_dot_v = max(dot(normal, view), 1e-4);
    float smooth = rikka_specular_occlusion(n_dot_v, visibility, roughness);
    float cone = rikka_cone_occlusion(bent, visibility, normal, reflect(-view, normal), roughness);
    float alpha = roughness * roughness;
    return cone + (smooth - cone) * (1.0 - smoothstep(0.01, 0.09, alpha));
}

float3 rikka_multi_bounce(float visibility, float3 albedo) {
    float3 a = 2.0404 * albedo - 0.3324;
    float3 b = -4.7951 * albedo + 0.6417;
    float3 c = 2.7552 * albedo + 0.6903;
    float x = visibility;
    return min(max(float3(x), ((x * a + b) * x + c) * x), float3(1.0));
}
