import fluid;

struct Push {
    Settings settings;
    ReadVec4s positions;
    ReadVec4s velocities;
    ReadUints phases;
    ReadUints offsets;
    Vec4s foam_positions;
    Vec4s foam_velocities;
    Uints foam_meta;
    Uints counters;
    uint count;
    uint capacity;
    uint seed;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    uint base = pc.counters[FOAM_COMPACTED];
    if (i == 0u) {
        uint made_total = pc.count > 0u ? pc.offsets[pc.count] : 0u;
        pc.counters[FOAM_COUNT] = min(base + made_total, pc.capacity);
    }
    if (i >= pc.count) {
        return;
    }
    uint first = pc.offsets[i];
    uint made = pc.offsets[i + 1u] - first;
    if (made == 0u) {
        return;
    }
    Settings s = pc.settings;
    float3 x = pc.positions[i].xyz;
    float3 v = pc.velocities[i].xyz;
    float speed = length(v);
    float3 along = speed > 0.0 ? v / speed : float3(0.0, 1.0, 0.0);
    float3 helper = abs(along.y) < 0.9 ? float3(0.0, 1.0, 0.0) : float3(1.0, 0.0, 0.0);
    float3 e1 = normalize(cross(along, helper));
    float3 e2 = cross(along, e1);
    float radius = s.bounds_lo.w;
    uint substance = substance_of(pc.phases[i]);
    for (uint k = 0u; k < made; ++k) {
        uint slot = base + first + k;
        if (slot >= pc.capacity) {
            return;
        }
        uint seed = (i * 8u + k) * 0x9e3779b9u + pc.seed;
        float spread = radius * sqrt(unit_random(seed));
        float angle = 6.2831853 * unit_random(seed ^ 0x68bc21ebu);
        float ahead = kernel_h(s) * unit_random(seed ^ 0x02e5be93u);
        float3 offset = spread * (cos(angle) * e1 + sin(angle) * e2);
        pc.foam_positions[slot] = float4(x + offset + along * ahead * min(speed * 0.05, 1.0), 1.0);
        pc.foam_velocities[slot] = float4(v + offset * (speed / max(radius, 1.0e-6)) * 0.1, 0.0);
        pc.foam_meta[slot] = FOAM_SPRAY | (substance << 8u);
    }
}
