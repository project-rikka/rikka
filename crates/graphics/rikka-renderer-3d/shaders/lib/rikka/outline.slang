__exported import rikka.common;

static const uint RIKKA_OUTLINE_NONE = 255u;

int2 rikka_outline_pixel(int2 pixel, float2 ratio) {
    return int2((float2(pixel) + 0.5) * ratio);
}

bool rikka_outlined(uint image, uint reach, int4 area, int2 pixel) {
    if (image == RIKKA_NONE
        || any(pixel < area.xy)
        || any(pixel >= area.zw)) {
        return false;
    }
    int span = int(reach);
    int first = max(pixel.y - span, area.y);
    int last = min(pixel.y + span, area.w - 1);
    for (int y = first; y <= last; ++y) {
        uint across = rikka_identity_images[image].Load(int3(pixel.x, y, 0)).r;
        if (across == RIKKA_OUTLINE_NONE) {
            continue;
        }
        int down = y - pixel.y;
        if (int(across * across) + down * down <= span * span) {
            return true;
        }
    }
    return false;
}
