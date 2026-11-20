import rikka.effects.layer_bloom;

[[vk::push_constant]] ConstantBuffer<RikkaBloomPush<Parameters>> push;

[shader("compute")]
[numthreads(8, 8, 1)]
void combine(uint3 id: SV_DispatchThreadID) {
    push.combine(id);
}
