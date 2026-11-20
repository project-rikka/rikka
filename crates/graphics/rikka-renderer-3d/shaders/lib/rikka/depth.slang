__exported import rikka.common;
__exported import rikka.push;
__exported import rikka.fragment;

[vk::constant_id(0)] const bool RIKKA_CUTOUT = false;

uint rikka_identity(RikkaInstance inst) {
    if ((inst.flags & RIKKA_PICKABLE) == 0u) {
        return 0u;
    }
    return ((inst.object_id + 1u) & RIKKA_IDENTITY_MASK)
        | ((inst.flags & RIKKA_SELECTED) != 0u ? RIKKA_IDENTITY_SELECTED : 0u);
}
