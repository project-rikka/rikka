import rikka.sky.fog;
import rikka.sky.noise;
import rikka.surface;

struct Push {
    RikkaFrameRef frame;
    uint target;
    uint pad;
    uint2 size;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const int STEPS = 64;
static const float DECAY = 0.985;
static const float REACH = 0.9;
static const float FOCUS = 4.0;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    int2 size = int2(push.size);
    int2 texel = int2(global_id.xy);
    if (any(texel >= size)) {
        return;
    }
    RikkaFrameRef frame = push.frame;
    RikkaViewRef view = frame.view;
    RikkaSky sky = frame.look.sky;
    RikkaShaftSource source = rikka_shaft_source(sky);
    float4 clip = mul(view.view_projection, float4(source.direction, 0.0));
    if (clip.w <= 1e-4) {
        rikka_storage_r32f[push.target][uint2(texel)] = 0.0;
        return;
    }
    float2 uv = (float2(texel) + 0.5) / float2(size);
    float2 centre = (clip.xy / clip.w) * 0.5 + 0.5;
    float2 toward = centre - uv;
    float2 aspect = float2(view.viewport.x * view.inverse_viewport.y, 1.0);
    float apart = length(toward * aspect);
    float2 step = toward * (min(1.0, REACH / max(apart, 1e-4)) / float(STEPS));
    float jitter = rikka_interleaved_gradient(float2(texel));
    int2 depth_size = int2(view.viewport);
    uint depth_image = frame.look.scene.depth;
    float sum = 0.0;
    float total = 0.0;
    float weight = 1.0;
    for (int i = 0; i < STEPS; ++i) {
        float2 at = uv + step * (float(i) + jitter);
        if (all(at >= float2(0.0)) && all(at < float2(1.0))) {
            int2 pixel = clamp(int2(at * view.viewport), int2(0), depth_size - 1);
            if (rikka_read_depth(depth_image, pixel) <= 0.0) {
                sum += weight * exp(-length((centre - at) * aspect) * FOCUS);
            }
        }
        total += weight;
        weight *= DECAY;
    }
    rikka_storage_r32f[push.target][uint2(texel)] = sum / total;
}
