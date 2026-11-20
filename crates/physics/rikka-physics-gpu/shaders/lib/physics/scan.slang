import glsl;
import physics.memory;

groupshared uint scan_totals[256];
groupshared uint scan_grand;

uint scan_exclusive(uint value) {
    uint inclusive = subgroupInclusiveAdd(value);
    if (gl_SubgroupInvocationID == gl_SubgroupSize - 1u) {
        shared_store(__getAddress(scan_totals[gl_SubgroupID]), inclusive);
    }
    group_barrier();
    if (gl_SubgroupID == 0u) {
        uint carry = 0u;
        for (uint base = 0u; base < gl_NumSubgroups; base += gl_SubgroupSize) {
            uint at = base + gl_SubgroupInvocationID;
            uint own = at < gl_NumSubgroups ? shared_load(__getAddress(scan_totals[at])) : 0u;
            uint scanned = subgroupInclusiveAdd(own);
            if (at < gl_NumSubgroups) {
                shared_store(__getAddress(scan_totals[at]), carry + scanned - own);
            }
            carry += subgroupBroadcast(scanned, gl_SubgroupSize - 1u);
        }
        if (subgroupElect()) {
            shared_store(__getAddress(scan_grand), carry);
        }
    }
    group_barrier();
    return shared_load(__getAddress(scan_totals[gl_SubgroupID])) + inclusive - value;
}

uint scan_total() {
    return shared_load(__getAddress(scan_grand));
}

void scan_quad(Uints values, uint base, uint count) {
    uint v0 = 0u;
    uint v1 = 0u;
    uint v2 = 0u;
    uint v3 = 0u;
    bool full = base + 3u < count;
    if (full) {
        v0 = values[base];
        v1 = values[base + 1u];
        v2 = values[base + 2u];
        v3 = values[base + 3u];
    } else {
        if (base < count) {
            v0 = values[base];
        }
        if (base + 1u < count) {
            v1 = values[base + 1u];
        }
        if (base + 2u < count) {
            v2 = values[base + 2u];
        }
    }
    uint running = scan_exclusive(v0 + v1 + v2 + v3);
    if (full) {
        values[base] = running;
        running += v0;
        values[base + 1u] = running;
        running += v1;
        values[base + 2u] = running;
        running += v2;
        values[base + 3u] = running;
    } else {
        if (base < count) {
            values[base] = running;
        }
        running += v0;
        if (base + 1u < count) {
            values[base + 1u] = running;
        }
        running += v1;
        if (base + 2u < count) {
            values[base + 2u] = running;
        }
    }
}
