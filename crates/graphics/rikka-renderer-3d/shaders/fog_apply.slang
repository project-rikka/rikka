import rikka.sky.fog;
import rikka.surface;

struct Push {
    RikkaFrameRef frame;
    uint source;
    uint shafts;
    uint shafts_divisor;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float SHAFT_GAIN = 1.0;
static const float SHAFT_HAZE = 0.08;
static const float SHAFT_DEPTH = 400.0;
static const float SHAFT_ANISOTROPY = 0.7;

float3 shafts(RikkaViewRef view, RikkaSky sky, float2 uv, float3 ray) {
    float medium = SHAFT_HAZE * sky.haze + 1.0 - exp(-sky.fog_density * SHAFT_DEPTH);
    if (push.shafts == RIKKA_NONE || sky.fog_shafts <= 0.0 || medium <= 0.0) {
        return float3(0.0);
    }
    RikkaShaftSource source = rikka_shaft_source(sky);
    float rays = rikka_images[push.shafts].SampleLevel(
        rikka_samplers[RIKKA_ATMO_SAMPLER_CLAMP], rikka_render_uv(view, uv, push.shafts_divisor), 0.0).r;
    float phase = rikka_henyey_greenstein(SHAFT_ANISOTROPY, dot(ray, source.direction));
    return source.color * (rays * phase * medium * sky.fog_shafts * SHAFT_GAIN);
}

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    int2 pixel = int2(frag_coord.xy);
    RikkaFrameRef frame = push.frame;
    RikkaViewRef view = frame.view;
    RikkaSky sky = frame.look.sky;
    float4 color = rikka_fetch(rikka_images[push.source], pixel, 0);
    float2 uv = (float2(pixel) + 0.5) * view.inverse_viewport;
    float3 ray = rikka_view_ray(view, uv * 2.0 - 1.0);
    float along = max(dot(ray, view.camera_forward), 1e-3);
    float depth = rikka_read_depth(frame.look.scene.depth, pixel);
    bool open = depth <= 0.0;
    float view_depth = open ? RIKKA_FOG_SKY_DISTANCE * along : rikka_linear_depth(view, depth);

    if (!open && sky.physical != 0u) {
        float4 aerial = rikka_aerial_at(sky, uv, view_depth / along * RIKKA_WORLD_TO_KM);
        color.rgb = color.rgb * aerial.a + aerial.rgb * color.a;
    }
    if (sky.fog_density > 0.0) {
        if (view_depth > RIKKA_FOG_FAR) {
            float3 start = view.camera_position + ray * (RIKKA_FOG_FAR / along);
            float4 beyond = rikka_height_fog(frame, sky, start, ray,
                                             (view_depth - RIKKA_FOG_FAR) / along);
            color.rgb = color.rgb * beyond.a + beyond.rgb * color.a;
        }
        float4 fog = rikka_fog_volume_at(sky, uv, view_depth);
        color.rgb = color.rgb * fog.a + fog.rgb * color.a;
    }
    color.rgb += shafts(view, sky, uv, ray) * color.a;
    return color;
}
