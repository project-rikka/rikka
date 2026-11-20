float float_sign(float x) {
    return spirv_asm { result:$$float = OpExtInst glsl450 FSign $x };
}

float floored_mod(float x, float y) {
    return spirv_asm { result:$$float = OpFMod $x $y };
}
