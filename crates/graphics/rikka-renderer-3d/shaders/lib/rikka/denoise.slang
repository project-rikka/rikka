__exported import rikka.surface;
__exported import rikka.temporal;

struct RikkaGuide {
    float3 normal;
    float depth;
    bool covered;
};

int2 rikka_full_pixel(uint2 texel, uint scale, uint2 size) {
    return min(int2(texel * scale), int2(size) - 1);
}

RikkaGuide rikka_guide(RikkaFrameRef frame, int2 pixel) {
    RikkaViewRef view = frame.view;
    RikkaScene scene = frame.look.scene;
    float depth = rikka_read_depth(scene.depth, pixel);
    RikkaGuide g;
    g.covered = depth > 0.0;
    float2 uv = (float2(pixel) + 0.5) * view.inverse_viewport;
    float3 world = rikka_world_at(view, uv, depth);
    g.depth = g.covered ? mul(view.view_projection, float4(world, 1.0)).w : 0.0;
    g.normal = rikka_read_surface(scene.surface, pixel).normal;
    return g;
}

float4 rikka_guide_texel(RikkaGuide g) {
    return float4(g.normal, g.covered ? g.depth : 0.0);
}

RikkaGuide rikka_guide_load(uint image, int2 texel) {
    float4 t = rikka_fetch(rikka_images[NonUniformResourceIndex(image)], texel, 0);
    RikkaGuide g;
    g.normal = t.xyz;
    g.depth = t.w;
    g.covered = t.w > 0.0;
    return g;
}

float3 rikka_world_of(RikkaFrameRef frame, int2 pixel) {
    RikkaViewRef view = frame.view;
    float depth = rikka_read_depth(frame.look.scene.depth, pixel);
    float2 uv = (float2(pixel) + 0.5) * view.inverse_viewport;
    return rikka_world_at(view, uv, depth);
}

float rikka_signal_luminance(float4 value, uint channels) {
    if (channels == 3u) {
        return rikka_luma(value.rgb);
    }
    if (channels == 2u) {
        return 0.5 * (value.r + value.g);
    }
    if (channels == 1u) {
        return value.r;
    }
    return 0.25 * (value.r + value.g + value.b + value.a);
}

float rikka_guide_weight(RikkaGuide here, float3 normal, float depth, float tolerance, float power) {
    if (!here.covered || depth <= 0.0) {
        return 0.0;
    }
    float along = exp(-abs(depth - here.depth) / max(tolerance * here.depth, 1e-4));
    float facing = pow(max(dot(here.normal, normal), 0.0), power);
    return along * facing;
}

static const uint RIKKA_TILE = 8u;
static const float RIKKA_TILE_MIXED = 2.0;

bool rikka_tile_settled(uint tiles, uint2 texel, uint2 size) {
    if (tiles == RIKKA_NONE) {
        return false;
    }
    int2 at = int2(texel / RIKKA_TILE);
    int2 highest = int2((size + RIKKA_TILE - 1u) / RIKKA_TILE) - 1;
    float4 centre = rikka_fetch(rikka_images[NonUniformResourceIndex(tiles)], at, 0);
    if (any(centre >= float4(RIKKA_TILE_MIXED))) {
        return false;
    }
    [unroll]
    for (int y = -1; y <= 1; ++y) {
        [unroll]
        for (int x = -1; x <= 1; ++x) {
            int2 tap = clamp(at + int2(x, y), int2(0), highest);
            float4 around = rikka_fetch(rikka_images[NonUniformResourceIndex(tiles)], tap, 0);
            if (any(around != centre)) {
                return false;
            }
        }
    }
    return true;
}
