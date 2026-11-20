import rikka.space.view;

struct Push {
    RikkaFrameRef frame;
    uint source;
};

[[vk::push_constant]] ConstantBuffer<Push> push;

static const float FAR = 1e6;

struct Probe {
    float distance;
    float3 normal;
    uint figure;
    bool covered;
};

Probe probe(SpaceView v, RikkaScene scene, int2 pixel) {
    int2 clamped = clamp(pixel, int2(0), int2(v.size) - 1);
    float depth = rikka_read_depth(scene.depth, clamped);
    RikkaSurfaceSample surface = rikka_read_surface(scene.surface, clamped);
    Probe p;
    p.covered = depth > 0.0;
    p.distance = p.covered ? space_distance(v, depth) : FAR;
    p.normal = surface.normal;
    p.figure = p.covered ? surface.figure : 0u;
    return p;
}

float pair_edge(Probe centre, Probe a, Probe b, float depth_gain, float crease_gain) {
    float bend = 2.0 - centre.distance / a.distance - centre.distance / b.distance;
    float edge = smoothstep(0.04, 0.12, bend * depth_gain);
    if (a.covered && b.covered) {
        float turn = 1.0 - dot(a.normal, b.normal);
        edge = max(edge, smoothstep(0.25, 0.7, turn * crease_gain));
    }
    return edge;
}

float boundary(Probe centre, Probe other) {
    bool differs = other.figure != centre.figure;
    bool nearer = centre.distance <= other.distance * 1.002;
    return differs && nearer ? 1.0 : 0.0;
}

[shader("fragment")]
float4 fragment_main(float4 frag_coord: SV_Position) : SV_Target {
    int2 pixel = int2(frag_coord.xy);
    float4 color = rikka_fetch(rikka_images[push.source], pixel, 0);
    RikkaFrameRef frame = push.frame;
    RikkaScene scene = frame.look.scene;
    RikkaSpace space = frame.look.space;
    SpaceView v = space_view(frame);
    Probe centre = probe(v, scene, pixel);
    if (!centre.covered) {
        return color;
    }
    float scale = space.lines_width * v.size.y / 1080.0;
    int reach = max(int(scale + 0.5), 1);
    float thin = clamp(scale, 0.0, 1.0);

    Probe left = probe(v, scene, pixel + int2(-reach, 0));
    Probe right = probe(v, scene, pixel + int2(reach, 0));
    Probe up = probe(v, scene, pixel + int2(0, -reach));
    Probe down = probe(v, scene, pixel + int2(0, reach));
    Probe up_left = probe(v, scene, pixel + int2(-reach, -reach));
    Probe down_right = probe(v, scene, pixel + int2(reach, reach));
    Probe up_right = probe(v, scene, pixel + int2(reach, -reach));
    Probe down_left = probe(v, scene, pixel + int2(-reach, reach));

    float depth_gain = space.lines_depth;
    float crease_gain = space.lines_crease;
    float edge = max(
        max(pair_edge(centre, left, right, depth_gain, crease_gain),
            pair_edge(centre, up, down, depth_gain, crease_gain)),
        max(pair_edge(centre, up_left, down_right, depth_gain, crease_gain),
            pair_edge(centre, up_right, down_left, depth_gain, crease_gain)) * 0.85);
    float outline = max(max(boundary(centre, left), boundary(centre, right)),
                        max(boundary(centre, up), boundary(centre, down)));
    edge = max(edge, outline);

    bool figure = centre.figure != 0u
        || left.figure != 0u || right.figure != 0u || up.figure != 0u || down.figure != 0u;
    float amount = clamp(edge * thin * space.lines_strength * (figure ? 1.0 : space.lines_environment), 0.0, 1.0);
    return float4(color.rgb * lerp(float3(1.0), space.lines_color, amount), color.a);
}
