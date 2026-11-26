#version 330
#extension GL_ARB_separate_shader_objects : require

uniform sampler2D MainSampler;

layout(location = 0) in vec2 texCoord;
layout(location = 0) out vec4 fragColor;

const float MIN_DIRECTION = 1.0 / 32768.0;
const float MIN_STEP = 1.0 / 65536.0;
const float WINDOW_SQUARED = 0.4;
const float WINDOW_SCALE = 25.0 / 16.0;
const float SHARP_LOBE = 0.21;

ivec2 limit;
ivec2 origin;
vec2 direction;
vec2 stretch;
float lobe;
float clip;

vec4 fetch(int x, int y) {
    return texelFetch(MainSampler, clamp(origin + ivec2(x, y), ivec2(0), limit), 0);
}

float luma(vec3 color) {
    return color.g + 0.5 * (color.r + color.b);
}

float edgeAlong(float before, float centre, float after, float across) {
    float steepest = max(max(abs(after - centre), abs(centre - before)), MIN_STEP);
    float edge = clamp(abs(across) / steepest, 0.0, 1.0);
    return edge * edge;
}

void analyse(inout vec2 gradient, inout float edge, float weight, float up, float left, float centre, float right, float down) {
    float horizontal = right - left;
    float vertical = down - up;
    gradient += vec2(horizontal, vertical) * weight;
    edge += (edgeAlong(left, centre, right, horizontal) + edgeAlong(up, centre, down, vertical)) * weight;
}

void accumulate(inout vec3 color, inout float total, vec2 offset, vec3 tap) {
    vec2 rotated = vec2(dot(offset, direction), dot(offset, vec2(-direction.y, direction.x))) * stretch;
    float distanceSquared = min(dot(rotated, rotated), clip);
    float window = WINDOW_SQUARED * distanceSquared - 1.0;
    float base = lobe * distanceSquared - 1.0;
    window *= window;
    base *= base;
    float weight = (WINDOW_SCALE * window - (WINDOW_SCALE - 1.0)) * base;
    color += tap * weight;
    total += weight;
}

void main() {
    ivec2 size = textureSize(MainSampler, 0);
    limit = size - 1;
    vec2 position = texCoord * vec2(size) - 0.5;
    vec2 cell = floor(position);
    vec2 inside = position - cell;
    origin = ivec2(cell);

    vec3 b = fetch(0, -1).rgb;
    vec3 c = fetch(1, -1).rgb;
    vec3 e = fetch(-1, 0).rgb;
    vec4 f = fetch(0, 0);
    vec4 g = fetch(1, 0);
    vec3 h = fetch(2, 0).rgb;
    vec3 i = fetch(-1, 1).rgb;
    vec4 j = fetch(0, 1);
    vec4 k = fetch(1, 1);
    vec3 l = fetch(2, 1).rgb;
    vec3 n = fetch(0, 2).rgb;
    vec3 o = fetch(1, 2).rgb;

    float lb = luma(b);
    float lc = luma(c);
    float le = luma(e);
    float lf = luma(f.rgb);
    float lg = luma(g.rgb);
    float lh = luma(h);
    float li = luma(i);
    float lj = luma(j.rgb);
    float lk = luma(k.rgb);
    float ll = luma(l);
    float ln = luma(n);
    float lo = luma(o);

    vec2 gradient = vec2(0.0);
    float edge = 0.0;
    analyse(gradient, edge, (1.0 - inside.x) * (1.0 - inside.y), lb, le, lf, lg, lj);
    analyse(gradient, edge, inside.x * (1.0 - inside.y), lc, lf, lg, lh, lk);
    analyse(gradient, edge, (1.0 - inside.x) * inside.y, lf, li, lj, lk, ln);
    analyse(gradient, edge, inside.x * inside.y, lg, lj, lk, ll, lo);

    float gradientSquared = dot(gradient, gradient);
    direction = gradientSquared < MIN_DIRECTION ? vec2(1.0, 0.0) : gradient * inversesqrt(gradientSquared);
    edge *= 0.5;
    edge *= edge;
    float squareStretch = 1.0 / max(abs(direction.x), abs(direction.y));
    stretch = vec2(1.0 + (squareStretch - 1.0) * edge, 1.0 - 0.5 * edge);
    lobe = 0.5 + (SHARP_LOBE - 0.5) * edge;
    clip = 1.0 / lobe;

    vec3 color = vec3(0.0);
    float total = 0.0;
    accumulate(color, total, vec2(0.0, -1.0) - inside, b);
    accumulate(color, total, vec2(1.0, -1.0) - inside, c);
    accumulate(color, total, vec2(-1.0, 0.0) - inside, e);
    accumulate(color, total, vec2(0.0, 0.0) - inside, f.rgb);
    accumulate(color, total, vec2(1.0, 0.0) - inside, g.rgb);
    accumulate(color, total, vec2(2.0, 0.0) - inside, h);
    accumulate(color, total, vec2(-1.0, 1.0) - inside, i);
    accumulate(color, total, vec2(0.0, 1.0) - inside, j.rgb);
    accumulate(color, total, vec2(1.0, 1.0) - inside, k.rgb);
    accumulate(color, total, vec2(2.0, 1.0) - inside, l);
    accumulate(color, total, vec2(0.0, 2.0) - inside, n);
    accumulate(color, total, vec2(1.0, 2.0) - inside, o);

    vec3 low = min(min(f.rgb, g.rgb), min(j.rgb, k.rgb));
    vec3 high = max(max(f.rgb, g.rgb), max(j.rgb, k.rgb));
    vec2 alphaRow = mix(vec2(f.a, j.a), vec2(g.a, k.a), inside.x);
    fragColor = vec4(clamp(color / total, low, high), mix(alphaRow.x, alphaRow.y, inside.y));
}
