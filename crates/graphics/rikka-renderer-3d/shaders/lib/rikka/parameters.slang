[__AttributeUsage(_AttributeTargets.Var)]
struct DefaultAttribute {
    float x;
    float y = 0.0;
    float z = 0.0;
    float w = 0.0;
};

[__AttributeUsage(_AttributeTargets.Var)]
struct PresetAttribute {
    string names;
    float x;
    float y = 0.0;
    float z = 0.0;
    float w = 0.0;
};

[__AttributeUsage(_AttributeTargets.Var)]
struct RangeAttribute {
    float low;
    float high;
};

[__AttributeUsage(_AttributeTargets.Var)]
struct SoftRangeAttribute {
    float low;
    float high;
};

[__AttributeUsage(_AttributeTargets.Var)]
struct GroupAttribute {
    string name;
};

[__AttributeUsage(_AttributeTargets.Var)]
struct OrderAttribute {
    int position;
};

[__AttributeUsage(_AttributeTargets.Var)]
struct LabelAttribute {
    string text;
};

[__AttributeUsage(_AttributeTargets.Var)]
struct ColorAttribute {
};

[__AttributeUsage(_AttributeTargets.Var)]
struct SrgbAttribute {
};

[__AttributeUsage(_AttributeTargets.Var)]
struct OptionsAttribute {
    string names;
};

[__AttributeUsage(_AttributeTargets.Var)]
struct TextureAttribute {
    string kind = "";
};

[__AttributeUsage(_AttributeTargets.Var)]
struct HintAttribute {
    string name;
};

[__AttributeUsage(_AttributeTargets.Var)]
struct FeatureAttribute {
    int bit;
};

[__AttributeUsage(_AttributeTargets.Var)]
struct RoleAttribute {
    string name;
};
