import rikka.light;

struct Push {
    RikkaFrameRef frame;
    uint inverse;
    uint amplitude;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void compute_main(uint3 global_id: SV_DispatchThreadID) {
    if (any(global_id.xy >= uint2(RIKKA_LTC_SIZE))) {
        return;
    }
    RikkaLtcEntry entry = push.frame.ltc[global_id.y * RIKKA_LTC_SIZE + global_id.x];
    rikka_storage_rgba16f[push.inverse][global_id.xy] = entry.inverse;
    rikka_storage_rgba16f[push.amplitude][global_id.xy] = entry.amplitude;
}
