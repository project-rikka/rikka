import plane;
import floating;

struct Push {
    Head head;
    float* shapes;
    float* inks;
    uint* tiles;
    uint* entries;
    uint across;
    uint light;
    uint dark;
    float square;
    float2 anchor;
}

[[vk::push_constant]] ConstantBuffer<Push> pc;

static const uint CAPSULE = 0u;
static const uint BAR = 1u;
static const uint TRIANGLE = 2u;
static const uint BOX = 3u;
static const uint OVAL = 4u;
static const uint SEGMENT = 5u;
static const uint EDGE = 6u;
static const uint STRIDE = 12u;
static const uint INK_STRIDE = 8u;
static const uint OVER = 0u;
static const uint LAYER = 1u;
static const uint SOLID = 0u;
static const uint BRUSH = 1u;
static const uint AREA = 2u;
static const uint KIND = 3u;
static const uint ERASE = 4u;
static const uint ALIASED = 8u;
static const uint EVEN_ODD = 16u;
static const float SPACING = 0.25;
static const float MIN_SPACING = 0.5;

float capsule(float2 p, float2 a, float2 b, float r) {
    float2 pa = p - a;
    float2 ba = b - a;
    float along = dot(ba, ba) > 0.0 ? clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0) : 0.0;
    return length(pa - ba * along) - r;
}

float bar(float2 p, float2 a, float2 b, float r) {
    float2 ba = b - a;
    float span = length(ba);
    float2 u = span > 0.0 ? ba / span : float2(1.0, 0.0);
    float2 n = float2(-u.y, u.x);
    float2 local = float2(dot(p - a, u) - span * 0.5, dot(p - a, n));
    float2 q = abs(local) - float2(span * 0.5 + r, r);
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0);
}

float triangle(float2 p, float2 a, float2 b, float2 c) {
    float2 e0 = b - a;
    float2 e1 = c - b;
    float2 e2 = a - c;
    float2 v0 = p - a;
    float2 v1 = p - b;
    float2 v2 = p - c;
    float2 pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    float2 pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    float2 pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    float s = float_sign(e0.x * e2.y - e0.y * e2.x);
    float2 d = min(min(float2(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)),
                       float2(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x))),
                       float2(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)));
    return -sqrt(d.x) * float_sign(d.y);
}

float box(float2 p, float2 low, float2 high) {
    float2 q = abs(p - (low + high) * 0.5) - (high - low) * 0.5;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0);
}

float oval(float2 p, float2 low, float2 high) {
    float2 radii = max((high - low) * 0.5, float2(1e-3));
    float2 q = p - (low + high) * 0.5;
    float k0 = length(q / radii);
    float k1 = length(q / (radii * radii));
    return k1 > 0.0 ? k0 * (k0 - 1.0) / k1 : -min(radii.x, radii.y);
}

float shape_distance(uint shape, float2 p) {
    uint at = shape * STRIDE;
    uint kind = uint(pc.shapes[at]);
    float r = pc.shapes[at + 2u];
    float2 a = float2(pc.shapes[at + 4u], pc.shapes[at + 5u]);
    float2 b = float2(pc.shapes[at + 6u], pc.shapes[at + 7u]);
    if (kind == CAPSULE) {
        return capsule(p, a, b, r);
    }
    if (kind == BAR) {
        return bar(p, a, b, r);
    }
    if (kind == BOX) {
        return box(p, a, b);
    }
    if (kind == OVAL) {
        return oval(p, a, b);
    }
    float2 c = float2(pc.shapes[at + 8u], pc.shapes[at + 9u]);
    return triangle(p, a, b, c);
}

float chord(float2 p, float2 a, float2 b, float r) {
    float2 ba = b - a;
    float span = length(ba);
    if (span <= 0.0) {
        return 0.0;
    }
    float2 u = ba / span;
    float2 pa = p - a;
    float along = dot(pa, u);
    float off = pa.x * u.y - pa.y * u.x;
    float half_chord = r * r - off * off;
    if (half_chord <= 0.0) {
        return 0.0;
    }
    half_chord = sqrt(half_chord);
    return max(min(along + half_chord, span) - max(along - half_chord, 0.0), 0.0);
}

float swept(float u) {
    if (u <= 0.0) {
        return u;
    }
    if (u < 1.0) {
        return u - 0.5 * u * u;
    }
    return 0.5;
}

float edge_area(float2 corner, float2 a, float2 b) {
    float s = 1.0;
    if (a.y > b.y) {
        float2 t = a;
        a = b;
        b = t;
        s = -1.0;
    }
    float y0 = max(a.y, corner.y);
    float y1 = min(b.y, corner.y + 1.0);
    if (y1 <= y0) {
        return 0.0;
    }
    float slope = (b.x - a.x) / (b.y - a.y);
    float u0 = a.x + (y0 - a.y) * slope - corner.x;
    float u1 = a.x + (y1 - a.y) * slope - corner.x;
    float covered = abs(u1 - u0) < 1e-4
        ? clamp(1.0 - 0.5 * (u0 + u1), 0.0, 1.0)
        : (swept(u1) - swept(u0)) / (u1 - u0);
    return s * (y1 - y0) * covered;
}

float4 ink_of(uint mark) {
    uint at = mark * INK_STRIDE;
    return float4(pc.inks[at], pc.inks[at + 1u], pc.inks[at + 2u], pc.inks[at + 3u]);
}

uint mode_of(uint mark) {
    return uint(pc.inks[mark * INK_STRIDE + 4u]);
}

float falloff(float d, float radius, float hardness, bool aliased) {
    float soft = radius * (1.0 - hardness);
    if (aliased && d >= radius) {
        return 0.0;
    }
    if (aliased && soft < 1.0) {
        return 1.0;
    }
    float edge = 0.5 * clamp(1.0 - soft, 0.0, 1.0);
    float width = max(soft, 1.0);
    float t = clamp((d - (radius + edge - width)) / width, 0.0, 1.0);
    return 1.0 - t * t * (3.0 - 2.0 * t);
}

float brushed(uint mark, float near, float travelled) {
    uint at = mark * INK_STRIDE;
    uint mode = mode_of(mark);
    float hardness = pc.inks[at + 5u];
    float flow = pc.inks[at + 6u];
    float radius = pc.inks[at + 7u];
    float shape = falloff(near, radius, hardness, (mode & ALIASED) != 0u);
    if (flow >= 1.0) {
        return shape;
    }
    float spacing = max(SPACING * 2.0 * radius, MIN_SPACING);
    float dabs = 1.0 + travelled / spacing;
    return shape * (1.0 - pow(1.0 - flow, dabs));
}

float filled(uint mark, float area) {
    uint mode = mode_of(mark);
    float winding = abs(area);
    float coverage = (mode & EVEN_ODD) != 0u
        ? abs(winding - 2.0 * floor(0.5 * winding + 0.5))
        : min(winding, 1.0);
    if ((mode & ALIASED) != 0u) {
        return coverage >= 0.5 ? 1.0 : 0.0;
    }
    return clamp(coverage, 0.0, 1.0);
}

float3 revealed(float2 p) {
    float2 square = floor((p - pc.anchor) / pc.square);
    bool lit = floored_mod(square.x + square.y, 2.0) < 0.5;
    return unpackUnorm4x8ToFloat(lit ? pc.light : pc.dark).rgb;
}

float4 over(float4 under, float4 ink, float coverage) {
    float a = ink.a * coverage;
    return float4(ink.rgb * a, a) + under * (1.0 - a);
}

struct Gathered {
    float solid;
    float near;
    float travelled;
    float area;
}

static const Gathered NOTHING_GATHERED = { 0.0, 1e30, 0.0, 0.0 };

float4 laid(float4 under, uint mark, Gathered gathered, float2 p) {
    uint mode = mode_of(mark);
    uint kind = mode & KIND;
    float coverage = kind == BRUSH
        ? brushed(mark, gathered.near, gathered.travelled)
        : kind == AREA ? filled(mark, gathered.area) : gathered.solid;
    if (coverage <= 0.0) {
        return under;
    }
    float4 ink = ink_of(mark);
    if ((mode & ERASE) == 0u) {
        return over(under, ink, coverage);
    }
    float a = ink.a * coverage;
    if (pc.head.mode == LAYER) {
        return float4(revealed(p) * a, a) + under * (1.0 - a);
    }
    return under * (1.0 - a);
}

[shader("compute")]
[numthreads(16, 16, 1)]
void marks(uint3 id : SV_DispatchThreadID) {
    int2 pixel = pc.head.pixel(id);
    bool inside = pc.head.dst.contains(pixel);
    float4 c = inside && pc.head.mode == OVER ? pc.head.src.load(pixel) : float4(0.0);
    float4 under = float4(c.rgb * c.a, c.a);
    int2 absolute = pixel + pc.head.window;
    uint2 tile = uint2(absolute) / 16u;
    uint index = tile.y * pc.across + tile.x;
    uint first = pc.tiles[index];
    uint last = pc.tiles[index + 1u];
    float2 corner = float2(absolute);
    float2 p = corner + 0.5;
    uint mark = 0xffffffffu;
    Gathered gathered = NOTHING_GATHERED;
    for (uint entry = first; entry < last; entry++) {
        uint shape = pc.entries[entry];
        uint at = shape * STRIDE;
        uint owner = uint(pc.shapes[at + 1u]);
        if (owner != mark) {
            if (mark != 0xffffffffu) {
                under = laid(under, mark, gathered, p);
            }
            mark = owner;
            gathered = NOTHING_GATHERED;
        }
        uint kind = uint(pc.shapes[at]);
        float2 a = float2(pc.shapes[at + 4u], pc.shapes[at + 5u]);
        float2 b = float2(pc.shapes[at + 6u], pc.shapes[at + 7u]);
        if (kind == EDGE) {
            gathered.area += edge_area(corner, a, b);
        } else if (kind == SEGMENT) {
            float r = pc.shapes[at + 2u];
            gathered.near = min(gathered.near, capsule(p, a, b, 0.0));
            gathered.travelled += chord(p, a, b, r);
        } else {
            gathered.solid = max(gathered.solid, clamp(0.5 - shape_distance(shape, p), 0.0, 1.0));
        }
    }
    if (mark != 0xffffffffu) {
        under = laid(under, mark, gathered, p);
    }
    if (!inside) {
        return;
    }
    if (pc.head.mode == LAYER) {
        pc.head.dst.store(pixel, clamp(under, 0.0, 1.0));
        return;
    }
    float4 result = float4(under.a > 0.0 ? under.rgb / under.a : float3(0.0), under.a);
    pc.head.dst.store(pixel, pc.head.clamped(result));
}
