import fluid;

struct Push {
    Settings settings;
    ReadVec4s positions;
    ReadUints phases;
    ReadUints starts;
    Uints list;
    Uints counts;
    uint count;
    uint stride;
};

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint COMPACT_REACH = 32768u;
static const uint MAX_RANGES = 18u;

struct Ranges : ICellVisitor {
    uint2 ranges[MAX_RANGES];
    uint count;
    uint lowest;
    uint highest;

    [mutating]
    void visit(uint first, uint last) {
        if (first < last) {
            ranges[count] = uint2(first, last);
            count += 1u;
            lowest = min(lowest, first);
            highest = max(highest, last - 1u);
        }
    }
}

uint emit_neighbours<let COMPACT_LIST : bool>(Ranges collected, uint i, float3 xi, float h2) {
    uint found = 0u;
    uint pending = 0u;
    uint cursor = i;
    for (uint r = 0u; r < collected.count; ++r) {
        uint2 range = collected.ranges[r];
        for (uint j = range.x; j < range.y; ++j) {
            if (j == i) {
                continue;
            }
            float3 d = xi - pc.positions[j].xyz;
            if (dot(d, d) >= h2) {
                continue;
            }
            if (COMPACT_LIST) {
                uint packed = uint(int(j) - int(i)) & 0xffffu;
                if ((found & 1u) == 0u) {
                    pending = packed;
                } else {
                    pc.list[cursor] = pending | (packed << 16u);
                    cursor += pc.stride;
                }
            } else {
                pc.list[cursor] = j;
                cursor += pc.stride;
            }
            found += 1u;
            if (found >= MAX_NEIGHBOURS) {
                break;
            }
        }
        if (found >= MAX_NEIGHBOURS) {
            break;
        }
    }
    if (COMPACT_LIST && (found & 1u) == 1u) {
        pc.list[cursor] = pending;
    }
    return found;
}

[shader("compute")]
[numthreads(128, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID) {
    uint i = thread.x;
    if (i >= pc.count) {
        return;
    }
    uint found = 0u;
    bool compact = true;
    if (!is_dead(pc.phases[i])) {
        Settings s = pc.settings;
        float h2 = kernel_h2(s);
        float3 xi = pc.positions[i].xyz;
        Ranges collected;
        collected.count = 0u;
        collected.lowest = i;
        collected.highest = i;
        for_cells(s, pc.starts, xi, collected);
        compact = i - collected.lowest < COMPACT_REACH && collected.highest - i < COMPACT_REACH;
        found = compact
            ? emit_neighbours<true>(collected, i, xi, h2)
            : emit_neighbours<false>(collected, i, xi, h2);
    }
    pc.counts[i] = found | (compact ? COMPACT : 0u);
}
