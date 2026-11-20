static const int POST_FOCUS_TILE = 8;

float post_circle_of_confusion(float depth, float scale, float ratio, float limit) {
    return clamp(scale * (1.0 - depth * ratio), -limit, limit);
}
