import rikka.depth;

[shader("fragment")]
void fragment_main(RikkaDepthVaryings varyings) {
    rikka_clip(rikka_push.frame, varyings.world_position);
}
