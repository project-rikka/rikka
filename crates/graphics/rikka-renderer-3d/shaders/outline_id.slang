import rikka.depth;

[shader("fragment")]
uint fragment_main(RikkaDepthVaryings varyings) : SV_Target {
    rikka_clip(rikka_push.frame, varyings.world_position);
    RikkaInstance inst = rikka_push.frame.instances[varyings.instance];
    return (inst.outline << RIKKA_OUTLINE_COLOR_SHIFT) | ((inst.object_id + 1u) & RIKKA_OUTLINE_OBJECT_MASK);
}
