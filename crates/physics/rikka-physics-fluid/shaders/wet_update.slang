import fluid;

struct Push {
    Uvec4s deposits;
    Vec4s state;
    Uints record;
    uint count;
    float keep;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint v = thread.x;
    if (v >= pc.count) {
        return;
    }
    uint4 deposit = pc.deposits[v];
    float4 added = float4(float3(deposit.yzw), float(deposit.x)) * (1.0 / WET_UNIT);
    float4 wet = pc.state[v] * pc.keep + added;
    if (wet.a > 1.0) {
        wet *= 1.0 / wet.a;
    }
    wet.rgb = min(wet.rgb, float3(wet.a));
    pc.state[v] = wet;
    pc.deposits[v] = uint4(0u);
    pc.record[v] = packUnorm4x8(wet);
}
