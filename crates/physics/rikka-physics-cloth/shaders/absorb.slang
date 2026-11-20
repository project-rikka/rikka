import cloth.common;
import physics.reaction;

struct Source : INonWritable {
    Reactions reactions;
    uint present;
    uint padding;

    static Source load_from<let ALIGN : int>(Source* address) {
        return {
            load_non_writable<16>(__getAddress(address->reactions)),
            load_non_writable<8>(__getAddress(address->present)),
            load_non_writable<4>(__getAddress(address->padding)),
        };
    }
};

typealias Sources = NonWritableBuffer<Source, 16>;

struct Push {
    Vec4s velocities;
    Vec4s traits;
    Uints starts;
    Uvec4s entries;
    Sources sources;
    uint count;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    float4 trait_a = pc.traits[4u * i];
    if (trait_a.x == 0.0) {
        return;
    }
    float inverse_mass = pc.traits[4u * i + 2u].w;
    float3 momentum = float3(0.0);
    uint to = pc.starts[i + 1u];
    for (uint at = pc.starts[i]; at < to; ++at) {
        uint4 entry = pc.entries[at];
        Source source = pc.sources[entry.x];
        if (source.present == 0u) {
            continue;
        }
        int4 pushed = source.reactions[entry.y];
        momentum += float3(pushed.xyz) * (REACTION_UNIT * asfloat(entry.z));
    }
    if (any(momentum != float3(0.0))) {
        float4 v = pc.velocities[i];
        pc.velocities[i] = float4(v.xyz + momentum * inverse_mass, v.w);
    }
}
