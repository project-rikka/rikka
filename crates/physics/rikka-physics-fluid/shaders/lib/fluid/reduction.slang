import glsl;
import physics.memory;

static const uint REDUCTION_ARRIVALS = 3u;
static const uint REDUCTION_HEADER = 4u;

typealias Sums = QueueCoherent<float>;

groupshared float totals[32];
groupshared bool closing;

float subgroup_sum(float value) {
    if (gl_SubgroupSize > 64u) {
        value += subgroupShuffleDown(value, 64u);
    }
    if (gl_SubgroupSize > 32u) {
        value += subgroupShuffleDown(value, 32u);
    }
    if (gl_SubgroupSize > 16u) {
        value += subgroupShuffleDown(value, 16u);
    }
    if (gl_SubgroupSize > 8u) {
        value += subgroupShuffleDown(value, 8u);
    }
    if (gl_SubgroupSize > 4u) {
        value += subgroupShuffleDown(value, 4u);
    }
    if (gl_SubgroupSize > 2u) {
        value += subgroupShuffleDown(value, 2u);
    }
    if (gl_SubgroupSize > 1u) {
        value += subgroupShuffleDown(value, 1u);
    }
    return value;
}

float group_sum(float value) {
    float lane = subgroup_sum(value);
    if (gl_SubgroupInvocationID == 0u) {
        shared_store(__getAddress(totals[gl_SubgroupID]), lane);
    }
    group_barrier();
    if (gl_SubgroupID == 0u) {
        float partial = 0.0;
        for (uint base = 0u; base < gl_NumSubgroups; base += gl_SubgroupSize) {
            uint at = base + gl_SubgroupInvocationID;
            if (at < gl_NumSubgroups) {
                partial += shared_load(__getAddress(totals[at]));
            }
        }
        return subgroup_sum(partial);
    }
    return 0.0;
}

float reduced(Sums sums, uint slot) {
    return sums[slot];
}

void reduce_groups(Sums sums, uint slot, float value, uint t) {
    uint* arrivals = (uint*)sums.address;
    uint groups = gl_NumWorkGroups.x;
    float partial = group_sum(value);
    bool leader = gl_SubgroupID == 0u && gl_SubgroupInvocationID == 0u;
    if (groups == 1u) {
        if (leader) {
            sums[slot] = partial;
        }
        return;
    }
    uint base = REDUCTION_HEADER + slot * groups;
    if (leader) {
        sums[base + gl_WorkGroupID.x] = partial;
        uint arrived = queue_arrive(arrivals + REDUCTION_ARRIVALS);
        shared_store(__getAddress(closing), arrived + 1u == groups);
    }
    buffer_barrier();
    if (!shared_load(__getAddress(closing))) {
        return;
    }
    float sum = 0.0;
    for (uint g = t; g < groups; g += 128u) {
        sum += sums[base + g];
    }
    float total = group_sum(sum);
    if (leader) {
        sums[slot] = total;
        queue_reset(arrivals + REDUCTION_ARRIVALS);
    }
}
