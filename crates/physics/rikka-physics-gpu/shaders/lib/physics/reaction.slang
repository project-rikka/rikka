import physics.memory;

static const float REACTION_UNIT = 1.0e-9;
static const uint REACTION_WORDS = 4u;

typealias Reactions = Ivec4s;
typealias ReactionWords = Buffer<int, 4>;
