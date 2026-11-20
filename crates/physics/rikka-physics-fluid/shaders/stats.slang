import glsl;
import fluid;
import fluid.bonds;

struct Push {
    ReadVec4s positions;
    ReadVec4s velocities;
    ReadUints phases;
    ReadUints bond_counts;
    ReadUints partners;
    ReadUints foam_counters;
    ReadUints tri_offsets;
    Uints stats;
    uint count;
    uint stride;
    uint triangles;
    uint bonds;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

groupshared uint live_sum[32];
groupshared uint fastest[32];
groupshared uint colours[32];
groupshared uint3 highest[32];
groupshared uint3 lowest[32];

uint ordered(float value) {
    uint bits = asuint(value);
    return (bits & 0x80000000u) != 0u ? ~bits : (bits | 0x80000000u);
}

uint3 subgroup_max3(uint3 value) {
    return uint3(subgroupMax(value.x), subgroupMax(value.y), subgroupMax(value.z));
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID, uint3 group : SV_GroupID) {
    uint i = thread.x;
    uint live = 0u;
    uint speed = 0u;
    uint3 high = uint3(0u);
    uint3 low = uint3(0u);
    uint coloured = 0u;
    if (i < pc.count && !is_dead(pc.phases[i])) {
        if (pc.bonds != 0u) {
            uint bond_count = pc.bond_counts[i];
            for (uint k = 0u; k < bond_count; ++k) {
                uint colour = bond_colour(pc.partners[k * pc.stride + i]);
                if (colour != UNCOLOURED) {
                    coloured = max(coloured, colour + 1u);
                }
            }
        }
        live = 1u;
        speed = asuint(length(pc.velocities[i].xyz));
        float3 x = pc.positions[i].xyz;
        high = uint3(ordered(x.x), ordered(x.y), ordered(x.z));
        low = uint3(ordered(-x.x), ordered(-x.y), ordered(-x.z));
    }
    uint sg_live = subgroupAdd(live);
    uint sg_speed = subgroupMax(speed);
    uint sg_colour = subgroupMax(coloured);
    uint3 sg_high = subgroup_max3(high);
    uint3 sg_low = subgroup_max3(low);
    if (gl_SubgroupInvocationID == 0u) {
        shared_store(__getAddress(live_sum[gl_SubgroupID]), sg_live);
        shared_store(__getAddress(fastest[gl_SubgroupID]), sg_speed);
        shared_store(__getAddress(colours[gl_SubgroupID]), sg_colour);
        shared_store(__getAddress(highest[gl_SubgroupID]), sg_high);
        shared_store(__getAddress(lowest[gl_SubgroupID]), sg_low);
    }
    group_barrier();
    if (gl_SubgroupID == 0u) {
        uint total_live = 0u;
        uint total_speed = 0u;
        uint total_colour = 0u;
        uint3 total_high = uint3(0u);
        uint3 total_low = uint3(0u);
        for (uint base = 0u; base < gl_NumSubgroups; base += gl_SubgroupSize) {
            uint at = base + gl_SubgroupInvocationID;
            if (at < gl_NumSubgroups) {
                total_live += shared_load(__getAddress(live_sum[at]));
                total_speed = max(total_speed, shared_load(__getAddress(fastest[at])));
                total_colour = max(total_colour, shared_load(__getAddress(colours[at])));
                total_high = max(total_high, shared_load(__getAddress(highest[at])));
                total_low = max(total_low, shared_load(__getAddress(lowest[at])));
            }
        }
        total_live = subgroupAdd(total_live);
        total_speed = subgroupMax(total_speed);
        total_colour = subgroupMax(total_colour);
        total_high = subgroup_max3(total_high);
        total_low = subgroup_max3(total_low);
        if (gl_SubgroupInvocationID == 0u) {
            if (total_live != 0u) {
                InterlockedAdd(*pc.stats.at(0), total_live);
                InterlockedMax(*pc.stats.at(1), total_speed);
                if (total_colour != 0u) {
                    InterlockedMax(*pc.stats.at(10), total_colour);
                }
                for (uint axis = 0u; axis < 3u; ++axis) {
                    InterlockedMax(*pc.stats.at(4u + axis), total_high[axis]);
                    InterlockedMax(*pc.stats.at(7u + axis), total_low[axis]);
                }
            }
            if (group.x == 0u) {
                pc.stats[2] = pc.triangles > 0u ? pc.tri_offsets[pc.triangles] : 0u;
                pc.stats[3] = pc.foam_counters[FOAM_COUNT];
            }
        }
    }
}
