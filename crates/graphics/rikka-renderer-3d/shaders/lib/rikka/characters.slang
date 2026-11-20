__exported import rikka.material.context;
__exported import rikka.surface;
static const int RIKKA_HAIR_SHADOW_TAPS = 8;

struct RikkaCharacterDepths {
    uint hair;
    uint face;
    uint scene;
    int2 extent;
};

RikkaCharacterDepths rikka_character_depths(RikkaContext context) {
    RikkaScene scene = context.frame.look.scene;
    RikkaCharacterDepths d;
    d.hair = scene.hair;
    d.face = scene.face;
    d.scene = scene.depth;
    d.extent = int2(context.view.viewport);
    return d;
}

float rikka_character_depth(RikkaViewRef view, uint image, int2 extent, float2 at) {
    int2 texel = clamp(int2(at), int2(0), extent - 1);
    float depth = rikka_read_depth(image, texel);
    return depth <= 0.0 ? 1e6 : rikka_linear_depth(view, depth);
}

float rikka_see_through(RikkaContext context, RikkaPixel pixel, float reach) {
    RikkaCharacterDepths d = rikka_character_depths(context);
    if (d.hair == RIKKA_NONE || d.face == RIKKA_NONE || d.scene == RIKKA_NONE || reach <= 0.0) {
        return 0.0;
    }
    RikkaViewRef view = context.view;
    float2 at = pixel.frag_coord.xy;
    float hair = rikka_character_depth(view, d.hair, d.extent, at);
    float front = rikka_character_depth(view, d.scene, d.extent, at);
    float face = rikka_character_depth(view, d.face, d.extent, at);
    float own = pixel.view_depth;
    float tolerance = 0.002 + 0.002 * own;
    bool hair_in_front = hair < 1e5 && hair <= front + tolerance;
    bool own_in_front = own <= face + tolerance;
    float behind = own - hair;
    if (!hair_in_front || !own_in_front || behind <= 0.0) {
        return 0.0;
    }
    return 1.0 - smoothstep(reach * 0.6, reach, behind);
}

float rikka_hair_shadow(
    RikkaContext context, float3 world_position, float view_depth, float3 to_light,
    float distance, float reach, float softness)
{
    RikkaCharacterDepths d = rikka_character_depths(context);
    if (d.hair == RIKKA_NONE || distance <= 0.0) {
        return 0.0;
    }
    RikkaViewRef view = context.view;
    float4 here = mul(view.view_projection, float4(world_position, 1.0));
    float4 there = mul(view.view_projection, float4(world_position + to_light * distance, 1.0));
    if (here.w <= 0.0 || there.w <= 0.0) {
        return 0.0;
    }
    float2 frag = (here.xy / here.w * 0.5 + 0.5) * view.viewport;
    float2 offset = (there.xy / there.w - here.xy / here.w) * 0.5 * view.viewport;
    float span = length(offset);
    float2 across = span > 1e-4 ? float2(-offset.y, offset.x) / span : float2(0.0);
    float spread = softness * max(span, 1.0);
    float own = view_depth;
    float bias = 0.002 + 0.001 * own;
    float shadow = 0.0;
    [unroll]
    for (int tap = 0; tap < RIKKA_HAIR_SHADOW_TAPS; ++tap) {
        float turn = (float(tap) + 0.5) / float(RIKKA_HAIR_SHADOW_TAPS);
        float along = 0.7 + 0.6 * frac(turn * 2.618);
        float side = turn * 2.0 - 1.0;
        float2 at = frag + offset * along + across * side * spread;
        float hair = rikka_character_depth(view, d.hair, d.extent, at);
        float gap = own - hair;
        shadow += gap > bias && gap < reach ? 1.0 : 0.0;
    }
    return smoothstep(0.2, 0.8, shadow / float(RIKKA_HAIR_SHADOW_TAPS));
}
