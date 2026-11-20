import fluid.settings;
import fluid.grid;

static const float WET_UNIT = 1048576.0;
static const uint FACE_VECS = 10u;
static const int SLAB_BELOW = 64;

struct Slab {
    int3 low;
    int3 high;
    int3 extent;
    int3 dims;
    int3 axes;
    float4 planes[2];
    float cell;
    float margin;
    bool culled;
};

float4 slab_plane(float3 a, float3 b, float3 c) {
    float3 n = cross(b - a, c - a);
    float area = length(n);
    if (area <= 0.0) {
        return float4(0.0);
    }
    n /= area;
    return float4(n, dot(n, a));
}

Slab triangle_slab(Settings s, ReadVec4s from, ReadVec4s to, uint4 tri) {
    float3 a0 = from[tri.x].xyz;
    float3 b0 = from[tri.y].xyz;
    float3 c0 = from[tri.z].xyz;
    float3 a1 = to[tri.x].xyz;
    float3 b1 = to[tri.y].xyz;
    float3 c1 = to[tri.z].xyz;
    float3 lo = min(min(min(a0, b0), c0), min(min(a1, b1), c1));
    float3 hi = max(max(max(a0, b0), c0), max(max(a1, b1), c1));
    Slab slab;
    slab.cell = s.tri_cell.x;
    slab.margin = s.tri_cell.w;
    slab.low = cell_of(lo - float3(slab.margin), s.tri_cell.y);
    slab.high = cell_of(hi + float3(slab.margin), s.tri_cell.y);
    slab.dims = int3(1 << s.tri_grid.x, 1 << s.tri_grid.y, 1 << s.tri_grid.z);
    int3 full = slab.high - slab.low + int3(1);
    slab.extent = min(full, slab.dims);
    slab.culled = full.x * full.y * full.z > SLAB_BELOW;
    if (slab.culled) {
        slab.planes[0] = slab_plane(a0, b0, c0);
        slab.planes[1] = slab_plane(a1, b1, c1);
        float3 n = abs(slab.planes[1].xyz);
        int c = n.x >= n.y && n.x >= n.z ? 0 : (n.y >= n.z ? 1 : 2);
        slab.axes = int3((c + 1) % 3, (c + 2) % 3, c);
    } else {
        slab.planes[0] = float4(0.0);
        slab.planes[1] = float4(0.0);
        slab.axes = int3(0, 1, 2);
    }
    return slab;
}

int2 slab_column(Slab slab, int a, int b) {
    int ca = slab.axes.z;
    int lo_c = slab.low[ca];
    int hi_c = slab.high[ca];
    int full_a = slab.high[slab.axes.x] - slab.low[slab.axes.x] + 1;
    int full_b = slab.high[slab.axes.y] - slab.low[slab.axes.y] + 1;
    float2 from_cell = float2(
        full_a > slab.dims[slab.axes.x] ? slab.low[slab.axes.x] : slab.low[slab.axes.x] + a,
        full_b > slab.dims[slab.axes.y] ? slab.low[slab.axes.y] : slab.low[slab.axes.y] + b);
    float2 to_cell = float2(
        full_a > slab.dims[slab.axes.x] ? slab.high[slab.axes.x] + 1 : slab.low[slab.axes.x] + a + 1,
        full_b > slab.dims[slab.axes.y] ? slab.high[slab.axes.y] + 1 : slab.low[slab.axes.y] + b + 1);
    float2 near = from_cell * slab.cell;
    float2 far = to_cell * slab.cell;
    float least = 1.0e30;
    float most = -1.0e30;
    bool bounded = true;
    for (int k = 0; k < 2; ++k) {
        float4 plane = slab.planes[k];
        float nc = plane[ca];
        if (abs(nc) < 0.3) {
            bounded = false;
            break;
        }
        float na = plane[slab.axes.x];
        float nb = plane[slab.axes.y];
        float spread = (abs(na) * (far.x - near.x) + abs(nb) * (far.y - near.y)) / abs(nc);
        float middle = (plane.w - na * 0.5 * (near.x + far.x) - nb * 0.5 * (near.y + far.y)) / nc;
        float reach = 0.5 * spread + slab.margin / abs(nc);
        least = min(least, middle - reach);
        most = max(most, middle + reach);
    }
    if (bounded) {
        lo_c = max(lo_c, int(floor(least / slab.cell)));
        hi_c = min(hi_c, int(floor(most / slab.cell)));
    }
    hi_c = min(hi_c, lo_c + slab.dims[ca] - 1);
    return int2(lo_c, hi_c);
}

int3 slab_cell(Slab slab, int a, int b, int c) {
    int3 cell;
    cell[slab.axes.x] = slab.low[slab.axes.x] + a;
    cell[slab.axes.y] = slab.low[slab.axes.y] + b;
    cell[slab.axes.z] = c;
    return cell;
}

struct SlabWalk {
    Slab slab;
    int a;
    int b;
    int c;
    int2 range;

    __init(Slab slab) {
        this.slab = slab;
        a = 0;
        b = -1;
        c = 0;
        range = int2(0, -1);
    }

    [mutating]
    bool next(out int3 cell) {
        c += 1;
        while (c > range.y) {
            b += 1;
            if (b >= slab.extent[slab.axes.y]) {
                b = 0;
                a += 1;
            }
            if (a >= slab.extent[slab.axes.x]) {
                cell = int3(0);
                return false;
            }
            range = slab_column(slab, a, b);
            c = range.x;
        }
        cell = slab_cell(slab, a, b, c);
        return true;
    }
}

uint slab_count(Slab slab) {
    if (!slab.culled) {
        return uint(slab.extent.x * slab.extent.y * slab.extent.z);
    }
    uint total = 0u;
    for (int a = 0; a < slab.extent[slab.axes.x]; ++a) {
        for (int b = 0; b < slab.extent[slab.axes.y]; ++b) {
            int2 range = slab_column(slab, a, b);
            total += uint(max(range.y - range.x + 1, 0));
        }
    }
    return total;
}
