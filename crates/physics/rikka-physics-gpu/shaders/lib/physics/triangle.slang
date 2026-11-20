struct Closest {
    float3 point;
    float3 weights;
    bool face;
};

Closest closest_on_triangle(float3 p, float3 a, float3 b, float3 c) {
    float3 ab = b - a;
    float3 ac = c - a;
    float3 ap = p - a;
    float d1 = dot(ab, ap);
    float d2 = dot(ac, ap);
    if (d1 <= 0.0 && d2 <= 0.0) {
        return Closest(a, float3(1.0, 0.0, 0.0), false);
    }
    float3 bp = p - b;
    float d3 = dot(ab, bp);
    float d4 = dot(ac, bp);
    if (d3 >= 0.0 && d4 <= d3) {
        return Closest(b, float3(0.0, 1.0, 0.0), false);
    }
    float vc = d1 * d4 - d3 * d2;
    if (vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0) {
        float v = d1 / (d1 - d3);
        return Closest(a + v * ab, float3(1.0 - v, v, 0.0), false);
    }
    float3 cp = p - c;
    float d5 = dot(ab, cp);
    float d6 = dot(ac, cp);
    if (d6 >= 0.0 && d5 <= d6) {
        return Closest(c, float3(0.0, 0.0, 1.0), false);
    }
    float vb = d5 * d2 - d1 * d6;
    if (vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0) {
        float w = d2 / (d2 - d6);
        return Closest(a + w * ac, float3(1.0 - w, 0.0, w), false);
    }
    float va = d3 * d6 - d5 * d4;
    if (va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0) {
        float w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return Closest(b + w * (c - b), float3(0.0, 1.0 - w, w), false);
    }
    float denominator = 1.0 / (va + vb + vc);
    float v = vb * denominator;
    float w = vc * denominator;
    return Closest(a + ab * v + ac * w, float3(1.0 - v - w, v, w), true);
}
