static const float RIKKA_PI = 3.14159265;
static const float RIKKA_TAU = 6.28318531;
static const float RIKKA_INV_PI = 0.31830988;

static const uint RIKKA_CLUSTER_TILE = 16u;
static const uint RIKKA_CLUSTER_SLICES = 24u;
static const uint RIKKA_NONE = 0xffffffffu;
static const uint RIKKA_SHADOW_UNMAPPED = 0x80000000u;

static const uint RIKKA_IDENTITY_SELECTED = 0x80000000u;
static const uint RIKKA_IDENTITY_MASK = 0x7fffffffu;

static const uint RIKKA_CLUSTER_MAX_LIGHTS = 128u;

static const uint RIKKA_LIGHT_DIRECTIONAL = 0u;
static const uint RIKKA_LIGHT_POINT = 1u;
static const uint RIKKA_LIGHT_SPOT = 2u;
static const uint RIKKA_LIGHT_RECT = 3u;
static const uint RIKKA_LIGHT_DISC = 4u;
static const uint RIKKA_LIGHT_SPHERE = 5u;
static const uint RIKKA_LIGHT_TUBE = 6u;

static const uint RIKKA_LIGHT_LEADING = 1u << 0;
static const uint RIKKA_LIGHT_TWO_SIDED = 1u << 1;
static const uint RIKKA_LIGHT_KEY = 1u << 2;

static const uint RIKKA_SHADOW_PERSPECTIVE = 1u << 0;
static const uint RIKKA_SHADOW_FIGURE = 1u << 1;
static const uint RIKKA_SHADOW_WITHOUT_MAPPED_FIGURES = 1u << 2;
static const uint RIKKA_SHADOW_CACHED = 1u << 3;
static const uint RIKKA_SHADOW_TINTED = 1u << 4;
static const uint RIKKA_SHADOW_DETAILED = 1u << 5;

static const uint RIKKA_FIGURE_MAPPED = 1u << 0;
static const uint RIKKA_FIGURE_TURNED_KEY = 1u << 1;

static const uint RIKKA_ALL_CHANNELS = 0xffffffffu;

static const uint RIKKA_CASTS_SHADOW = 1u << 0;
static const uint RIKKA_RECEIVES_SHADOW = 1u << 1;
static const uint RIKKA_PICKABLE = 1u << 2;
static const uint RIKKA_SELECTED = 1u << 3;
static const uint RIKKA_NO_OCCLUSION = 1u << 4;
static const uint RIKKA_TRANSPARENT = 1u << 5;
static const uint RIKKA_DOUBLE_SIDED = 1u << 6;
static const uint RIKKA_DIRECT = 1u << 7;
static const uint RIKKA_MASKED = 1u << 8;
static const uint RIKKA_INDEX16 = 1u << 9;
static const uint RIKKA_DEFORMED = 1u << 10;
static const uint RIKKA_FACIAL = 1u << 11;

static const uint RIKKA_PASS_MASKED = 1u << 0;
static const uint RIKKA_PASS_SHADOW = 1u << 1;
static const uint RIKKA_PASS_LATE = 1u << 2;
static const uint RIKKA_PASS_SEE_THROUGH = 1u << 3;

[[vk::binding(0, 0)]] Sampler2D rikka_textures[];
[[vk::binding(1, 0)]] Texture2D rikka_images[];

[[vk::binding(1, 0)]] Texture2D<uint4> rikka_identity_images[];
[[vk::binding(2, 0)]] SamplerState rikka_samplers[];
[[vk::binding(3, 0)]] [[vk::image_format("rgba16f")]] RWTexture2D<float4> rikka_storage_rgba16f[];

[[vk::binding(3, 0)]] [[vk::image_format("r32f")]] RWTexture2D<float> rikka_storage_r32f[];

static const uint RIKKA_SAMPLER_SHADOW = 5u;
static const uint RIKKA_SAMPLER_DEPTH_REDUCE = 6u;
[[vk::binding(2, 0)]] SamplerComparisonState rikka_shadow_samplers[];

struct RikkaLight {
    float3 position;
    float range;
    float3 direction;
    float intensity;
    float3 color;
    uint kind;
    float3 tangent;
    float cone_scale;
    float2 extent;
    float cone_offset;
    uint shadow;
    uint shadow_count;
    uint characters;
    uint character_count;
    uint channels;
    uint figure;
    uint flags;
    float size;
    uint profile;
};

struct RikkaShadowSlot {
    float4x4 view_projection;
    float2 uv_scale;
    float2 uv_offset;
    float depth_bias;
    float normal_bias;
    float split_depth;
    float texel_size;
    uint figure;
    uint channels;
    uint flags;
    float near;
    float projection_scale;
    float depth_range;
    float world_texel;
    uint pad;
};

struct RikkaMaterial {
    uint block;
    float alpha_cutoff;
    uint albedo;
    uint albedo_texture;
    uint emission;
    uint emission_texture;
    uint roles;
    uint albedo_weight;
    uint metalness;
    uint metalness_texture;
    uint transmission;
    uint emission_luminance;
};

static const float RIKKA_NITS_PER_UNIT = 32000.0;

static const uint RIKKA_BLOCK_ALIGNMENT = 16u;

struct RikkaInstance {
    uint64_t positions;
    uint64_t attributes;
    float4x4 transform;
    float3x3 normal_matrix;
    float3 center;
    float radius;
    uint material;
    uint vertex_offset;
    uint first_index;
    uint index_count;
    uint object_id;
    uint flags;
    uint bucket;
    uint sort_key;
    uint figure;
    uint shadow_class;
    uint layers;
    uint64_t wet;
    uint64_t outline_normals;
    uint light_channels;
    uint outline;
};

struct RikkaAttribute {
    uint normal;
    uint tangent;
    uint uv;
    uint color;
};

struct RikkaCluster {
    uint count;
};

struct RikkaBucket {
    uint first;
    uint capacity;
};

struct RikkaDrawCommand {
    uint index_count;
    uint instance_count;
    uint first_index;
    int vertex_offset;
    uint first_instance;
};

struct RikkaView {
    float4x4 view;
    float4x4 projection;
    float4x4 view_projection;
    float4x4 inverse_view_projection;
    float4x4 inverse_projection;
    float4 frustum[6];
    float3 camera_position;
    float near;
    float3 camera_forward;
    float2 viewport;
    float2 inverse_viewport;
    uint3 cluster_counts;
    uint cluster_tile;
    float cluster_depth_scale;
    float cluster_depth_bias;
    uint light_count;
    uint directional_count;
    uint instance_count;
    float2 hzb_size;
    uint hzb_levels;
    float exposure;
    float3 ambient;
    float time;
    uint shadow_atlas;
    uint hzb_image;
    uint cluster_stride;
    uint shadow_classes;
    uint shadow_tint;
    uint shadow_tint_depth;
    float shadow_atlas_size;
    float2 jitter;
    float mip_bias;
    float render_scale;
    float2 previous_jitter;
    uint cut;
    uint shadow_noise;
    uint figure_atlas;
    float2 capacity;
    float2 previous_viewport;
};

struct RikkaFigure {
    float3 head_center;
    float head_radius;
    float3 head_forward;
    uint has_head;
    float3 head_up;
    float pad;
    uint shadow;
    uint key_light;
    uint flags;
    uint pad_flags;
};

struct RikkaScene {
    float4x4 previous_view_projection;
    uint depth;
    uint surface;
    uint figure_count;
    uint enabled;
    uint history;
    uint styled;
    float stylization;
    float shade_softness;
    float3 shadow_tint;
    float ambient;
    float previous_time;
    uint hair;
    uint face;
    uint brdf;
    uint scatter;
    uint refraction;
    uint ltc_inverse;
    uint ltc_amplitude;
    float4x4 previous_sky_view_projection;
};

struct RikkaSky {
    float3 to_sun;
    float sun_angular_radius;
    float3 sun_color;
    float haze;
    float3 to_moon;
    float moon_illumination;
    float3 moon_color;
    float stars;
    float3 ground;
    float clouds;
    float3 fog_color;
    float fog_density;
    float fog_height;
    float fog_forward;
    float fog_shafts;
    float cloud_drift;
    uint transmittance_lut;
    uint multiscatter_lut;
    uint sky_view_lut;
    uint aerial_lut;
    uint environment_image;
    uint environment_levels;
    uint fog_volume;
    uint physical;
    float3 sun_illuminance;
    float moon_angular_radius;
    float3 moon_illuminance;
    float aerial_range;
    float3x3 star_frame;
    uint3 fog_grid;
    uint environment;
    uint panorama;
    float panorama_background;
    float panorama_blur;
    float2 panorama_rotation;
    float3 panorama_scale;
    float panorama_clamp;
};

struct RikkaSpace {
    uint occlusion_image;
    float occlusion_strength;
    float occlusion_radius;
    float reflection_strength;
    float roughness_limit;
    float lines_strength;
    float lines_width;
    float lines_depth;
    float lines_crease;
    float lines_environment;
    float3 lines_color;
    uint particle_count;
    uint particle_kind;
    uint particle_seed;
    float particle_size;
    float particle_glow;
    float particle_reach;
    float particle_focus;
    float3 particle_color;
    float3 particle_wind;
    uint screen_shadow_image;
    float screen_shadow_strength;
    float screen_shadow_thickness;
    uint3 pad;
};

struct RikkaFluidParticle {
    float4 center;
    float4 rotation;
    float4 radii;
    float4 velocity;
};

struct RikkaFluidSpray {
    float4 position;
    float4 velocity;
};

struct RikkaFluidLayer {
    uint64_t particles;
    uint64_t spray;
    uint count;
    uint spray_count;
    uint material_first;
    uint material_count;
    float radius;
    float volume;
    uint pad0;
    uint pad1;
};

struct RikkaFluidMaterial {
    float3 absorption;
    float scattering;
    float3 albedo;
    float roughness;
    float ior;
    float extinction;
    float pad0;
    float pad1;
};

struct RikkaFluidView {
    float4x4 view_projection;
    float4x4 inverse;
    float2 texel;
    uint perspective;
    uint pad;
};

typealias RikkaFluidParticleRef = RikkaFluidParticle*;
typealias RikkaFluidSprayRef = RikkaFluidSpray*;
typealias RikkaWetRef = float4*;
typealias RikkaFluidLayerRef = RikkaFluidLayer*;
typealias RikkaFluidMaterialRef = RikkaFluidMaterial*;
typealias RikkaFluidViewRef = RikkaFluidView*;

struct RikkaFluid {
    RikkaFluidLayerRef layers;
    RikkaFluidMaterialRef materials;
    RikkaFluidViewRef views;
    float4x4 previous_view_projection;
    uint depth;
    uint front;
    uint smoothed;
    uint thickness;
    uint scattering;
    uint scene_depth;
    uint layer_count;
    uint view_count;
    float radius;
    float delta_time;
    float smoothing;
    uint coverage;
};

static const uint RIKKA_TRACED_LIGHTS = 4u;
static const uint RIKKA_UNTRACED = 0xffffffffu;
static const uint RIKKA_TRACED_LEVELS = 254u;
static const uint RIKKA_TRACED_UNKNOWN = 0xffu;

uint rikka_traced_pack(float4 visibility) {
    uint4 levels = uint4(round(saturate(visibility) * float(RIKKA_TRACED_LEVELS)));
    return levels.x | (levels.y << 8u) | (levels.z << 16u) | (levels.w << 24u);
}

float rikka_traced_unpack(uint traced, uint channel) {
    uint level = (traced >> (channel * 8u)) & 0xffu;
    return level == RIKKA_TRACED_UNKNOWN ? -1.0 : float(level) / float(RIKKA_TRACED_LEVELS);
}

struct RikkaTrace {
    uint2 scene;
    uint2 parts;
    uint2 indices16;
    uint2 indices32;
    uint shadow_lights[RIKKA_TRACED_LIGHTS];
    uint lights[RIKKA_TRACED_LIGHTS];
    uint visibility_image;
    uint indirect_image;
    uint radiance_image;
    uint frame;
    float indirect_strength;
    float reach;
    uint2 hooks;
    uint shadow_samples;
    uint _pad[3];
};

struct RikkaHookJob {
    uint instance;
    uint vertex_count;
    uint2 positions;
    uint2 attributes;
};

uint64_t rikka_address(uint2 words) {
    return (uint64_t(words.y) << 32) | uint64_t(words.x);
}

struct RikkaLook {
    RikkaSky sky;
    RikkaSpace space;
    RikkaScene scene;
    RikkaTrace trace;
};

struct RikkaEnvironment {
    float4 irradiance[9];
};

typealias RikkaViewRef = RikkaView*;
typealias RikkaInstanceRef = RikkaInstance*;
typealias RikkaMaterialRef = RikkaMaterial*;
typealias RikkaLightRef = RikkaLight*;
typealias RikkaShadowRef = RikkaShadowSlot*;
typealias RikkaBoneRef = float4x4*;
typealias RikkaPositionRef = float3*;
typealias RikkaTransformRef = float4x4*;
typealias RikkaAttributeRef = RikkaAttribute*;
typealias RikkaSkinRef = uint4*;
typealias RikkaFloatRef = float*;
typealias RikkaClusterRef = RikkaCluster*;
typealias RikkaIndexListRef = uint*;
typealias RikkaCounterRef = uint*;
typealias RikkaBucketRef = RikkaBucket*;
typealias RikkaCommandRef = RikkaDrawCommand*;
typealias RikkaLookRef = RikkaLook*;
typealias RikkaFigureRef = RikkaFigure*;
typealias RikkaEnvironmentRef = RikkaEnvironment*;
typealias RikkaFluidRef = RikkaFluid*;

struct RikkaLtcEntry {
    float4 inverse;
    float4 amplitude;
};

typealias RikkaLtcRef = RikkaLtcEntry*;

typealias RikkaProfileRef = float*;

struct RikkaFrame {
    RikkaViewRef view;
    RikkaInstanceRef instances;
    RikkaMaterialRef materials;
    RikkaLightRef lights;
    RikkaShadowRef shadows;
    RikkaBoneRef bones;
    RikkaClusterRef clusters;
    RikkaIndexListRef cluster_indices;
    RikkaBucketRef buckets;
    RikkaCommandRef draw_commands;
    RikkaCounterRef draw_counts;
    RikkaIndexListRef late_instances;
    RikkaCounterRef late_counter;
    RikkaLookRef look;
    RikkaFigureRef figures;
    RikkaEnvironmentRef environment;
    RikkaPositionRef previous_positions;
    RikkaFluidRef fluid;
    uint64_t blocks;
    RikkaTransformRef previous_transforms;
    RikkaLtcRef ltc;
    RikkaProfileRef profiles;
    uint64_t editor;
};

typealias RikkaFrameRef = RikkaFrame*;

static const float RIKKA_WET_FILM_ROUGHNESS = 0.08;
static const float RIKKA_WET_FILM_F0 = 0.02;

float4 rikka_wet_of(RikkaInstance inst, uint vertex_index) {
    return inst.wet != 0ul ? ((RikkaWetRef)inst.wet)[vertex_index - inst.vertex_offset] : float4(0.0);
}

float rikka_mod(float x, float y) {
    return spirv_asm { OpFMod $$float result $x $y };
}

float3 rikka_mod(float3 x, float3 y) {
    return spirv_asm { OpFMod $$float3 result $x $y };
}

float3x3 rikka_columns(float3 c0, float3 c1, float3 c2) {
    return transpose(float3x3(c0, c1, c2));
}

float4x4 rikka_columns(float4 c0, float4 c1, float4 c2, float4 c3) {
    return transpose(float4x4(c0, c1, c2, c3));
}

float4 rikka_column(float4x4 m, uint index) {
    return float4(m[0][index], m[1][index], m[2][index], m[3][index]);
}

float3 rikka_column(float3x3 m, uint index) {
    return float3(m[0][index], m[1][index], m[2][index]);
}

uint2 rikka_image_size(Texture2D image, uint level) {
    uint width;
    uint height;
    uint levels;
    image.GetDimensions(level, width, height, levels);
    return uint2(width, height);
}

uint2 rikka_image_size(Texture2D<uint4> image, uint level) {
    uint width;
    uint height;
    uint levels;
    image.GetDimensions(level, width, height, levels);
    return uint2(width, height);
}

uint2 rikka_image_size(RWTexture2D<float4> image) {
    uint width;
    uint height;
    image.GetDimensions(width, height);
    return uint2(width, height);
}

uint2 rikka_image_size(RWTexture2D<float> image) {
    uint width;
    uint height;
    image.GetDimensions(width, height);
    return uint2(width, height);
}

float4 rikka_fetch(Texture2D image, int2 texel, int level) {
    return image.Load(int3(texel, level));
}

float rikka_luma(float3 color) {
    return dot(color, float3(0.2126, 0.7152, 0.0722));
}

float rikka_interleaved_gradient(float2 pixel) {
    return fract(52.9829189 * fract(dot(pixel, float2(0.06711056, 0.00583715))));
}

float rikka_temporal_noise(RikkaViewRef view, float2 pixel) {
    float2 phase = view.jitter * view.viewport;
    return fract(rikka_interleaved_gradient(pixel) + dot(phase, float2(0.7548776662, 0.5698402910)) * 4.0);
}

float2 rikka_divided(float2 size, uint divisor) {
    return ceil(size / float(divisor));
}

float2 rikka_placed_uv(float2 uv, float2 size, float2 capacity) {
    if (all(size == capacity)) {
        return uv;
    }
    return clamp(uv * size, float2(0.5), size - 0.5) / capacity;
}

float2 rikka_render_uv(RikkaViewRef view, float2 uv, uint divisor) {
    return rikka_placed_uv(uv, rikka_divided(view.viewport, divisor), rikka_divided(view.capacity, divisor));
}

float2 rikka_previous_size(RikkaViewRef view, uint divisor) {
    return rikka_divided(view.previous_viewport, divisor);
}

float2 rikka_previous_render_uv(RikkaViewRef view, float2 uv, uint divisor) {
    return rikka_placed_uv(uv, rikka_previous_size(view, divisor), rikka_divided(view.capacity, divisor));
}

uint rikka_pcg_hash(uint value) {
    uint state = value * 747796405u + 2891336453u;
    uint word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

uint rikka_hash(uint x) {
    x ^= x >> 16;
    x *= 0x7feb352du;
    x ^= x >> 15;
    x *= 0x846ca68bu;
    x ^= x >> 16;
    return x;
}

float rikka_unit(uint x) {
    return float(x >> 8) * (1.0 / 16777216.0);
}

float3 rikka_encode_srgb(float3 linear) {
    float3 low = linear * 12.92;
    float3 high = 1.055 * pow(max(linear, float3(1e-5)), float3(1.0 / 2.4)) - 0.055;
    return select(linear <= float3(0.0031308), low, high);
}

float3 rikka_decode_srgb(float3 encoded) {
    float3 low = encoded / 12.92;
    float3 high = pow(max((encoded + 0.055) / 1.055, float3(0.0)), float3(2.4));
    return select(encoded <= float3(0.04045), low, high);
}

float3 rikka_decode_octahedral(uint packed) {
    float2 e = float2(
        float(int(packed << 16) >> 16) / 32767.0,
        float(int(packed) >> 16) / 32767.0);
    float3 n = float3(e, 1.0 - abs(e.x) - abs(e.y));
    if (n.z < 0.0) {

        float2 sign_xy = float2(n.x >= 0.0 ? 1.0 : -1.0, n.y >= 0.0 ? 1.0 : -1.0);
        n.xy = (1.0 - abs(float2(n.y, n.x))) * sign_xy;
    }
    return normalize(n);
}

float4 rikka_decode_tangent(uint packed) {

    float sign = (packed & 1u) != 0u ? -1.0 : 1.0;
    return float4(rikka_decode_octahedral(packed), sign);
}

uint rikka_encode_octahedral(float3 normal) {
    float3 n = normal / max(abs(normal.x) + abs(normal.y) + abs(normal.z), 1e-8);
    float2 e = n.xy;
    if (n.z < 0.0) {
        float2 sign_xy = float2(e.x >= 0.0 ? 1.0 : -1.0, e.y >= 0.0 ? 1.0 : -1.0);
        e = (1.0 - abs(float2(e.y, e.x))) * sign_xy;
    }
    int2 q = int2(round(clamp(e, -1.0, 1.0) * 32767.0));
    return (uint(q.x) & 0xffffu) | ((uint(q.y) & 0xffffu) << 16);
}

uint rikka_encode_tangent(float4 tangent) {
    uint packed = rikka_encode_octahedral(tangent.xyz) & ~1u;
    return packed | (tangent.w < 0.0 ? 1u : 0u);
}

float2 rikka_decode_uv(uint packed) {
    return unpackHalf2x16ToFloat(packed);
}

float4 rikka_decode_color(uint packed) {
    return unpackUnorm4x8ToFloat(packed);
}

float2 rikka_uv_transform(float2 uv, float2 offset, float2 scale, float rotation) {
    float2 scaled = uv * scale;
    if (rotation == 0.0) {
        return scaled + offset;
    }
    float c = cos(rotation);
    float s = sin(rotation);
    return float2(c * scaled.x + s * scaled.y, c * scaled.y - s * scaled.x) + offset;
}

uint rikka_cluster_slice(RikkaViewRef view, float view_depth) {
    float slice = log2(max(view_depth, view.near)) * view.cluster_depth_scale
        + view.cluster_depth_bias;
    return uint(clamp(slice, 0.0, float(view.cluster_counts.z - 1u)));
}

uint rikka_cluster_index(RikkaViewRef view, float2 frag_xy, float view_depth) {
    uint2 tile = uint2(frag_xy) / view.cluster_tile;
    tile = min(tile, view.cluster_counts.xy - 1u);
    uint slice = rikka_cluster_slice(view, view_depth);
    return (slice * view.cluster_counts.y + tile.y) * view.cluster_counts.x + tile.x;
}

float3 rikka_view_ray(RikkaViewRef view, float2 ndc) {
    float4 on_near = mul(view.inverse_view_projection, float4(ndc, 1.0, 1.0));
    return normalize(on_near.xyz / on_near.w - view.camera_position);
}

bool rikka_sphere_in_frustum(RikkaViewRef view, float3 center, float radius) {
    [unroll]
    for (int i = 0; i < 6; ++i) {
        if (dot(view.frustum[i].xyz, center) + view.frustum[i].w < -radius) {
            return false;
        }
    }
    return true;
}
