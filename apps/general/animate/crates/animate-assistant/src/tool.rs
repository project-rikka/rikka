use rikka_assist::KitTool;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Tool {
    Scene,
    Figure,
    Pose,
    Look,
    Nodes,
    EditGraph,
    Materials,
    LookKeys,
    ResetLook,
    Camera,
    Keys,
    Timeline,
    Render,
    Cloth,
    SetCloth,
    Wind,
    SetWind,
}

pub(crate) static TOOLS: [KitTool; 17] = [
    KitTool {
        stem: "scene",
        document: include_str!("../../../prompts/tools/scene.md"),
        gist: &[],
    },
    KitTool {
        stem: "figure",
        document: include_str!("../../../prompts/tools/figure.md"),
        gist: &["figure"],
    },
    KitTool {
        stem: "pose",
        document: include_str!("../../../prompts/tools/pose.md"),
        gist: &["figure"],
    },
    KitTool {
        stem: "look",
        document: include_str!("../../../prompts/tools/look.md"),
        gist: &["graph", "figure"],
    },
    KitTool {
        stem: "nodes",
        document: include_str!("../../../prompts/tools/nodes.md"),
        gist: &["graph", "search"],
    },
    KitTool {
        stem: "edit_graph",
        document: include_str!("../../../prompts/tools/edit_graph.md"),
        gist: &["graph", "edits[]"],
    },
    KitTool {
        stem: "materials",
        document: include_str!("../../../prompts/tools/materials.md"),
        gist: &["add[].name", "assign[].figure"],
    },
    KitTool {
        stem: "look_keys",
        document: include_str!("../../../prompts/tools/look_keys.md"),
        gist: &["set[].frame", "remove[]"],
    },
    KitTool {
        stem: "reset_look",
        document: include_str!("../../../prompts/tools/reset_look.md"),
        gist: &["world", "composite"],
    },
    KitTool {
        stem: "camera",
        document: include_str!("../../../prompts/tools/camera.md"),
        gist: &["keys[].frame"],
    },
    KitTool {
        stem: "keys",
        document: include_str!("../../../prompts/tools/keys.md"),
        gist: &["action", "subject"],
    },
    KitTool {
        stem: "timeline",
        document: include_str!("../../../prompts/tools/timeline.md"),
        gist: &["frame"],
    },
    KitTool {
        stem: "render",
        document: include_str!("../../../prompts/tools/render.md"),
        gist: &["frames[]"],
    },
    KitTool {
        stem: "cloth",
        document: include_str!("../../../prompts/tools/cloth.md"),
        gist: &["figure"],
    },
    KitTool {
        stem: "set_cloth",
        document: include_str!("../../../prompts/tools/set_cloth.md"),
        gist: &["figure", "pieces[].piece"],
    },
    KitTool {
        stem: "wind",
        document: include_str!("../../../prompts/tools/wind.md"),
        gist: &[],
    },
    KitTool {
        stem: "set_wind",
        document: include_str!("../../../prompts/tools/set_wind.md"),
        gist: &["keys[].frame"],
    },
];

impl Tool {
    pub(crate) const ALL: [Self; 17] = [
        Self::Scene,
        Self::Figure,
        Self::Pose,
        Self::Look,
        Self::Nodes,
        Self::EditGraph,
        Self::Materials,
        Self::LookKeys,
        Self::ResetLook,
        Self::Camera,
        Self::Keys,
        Self::Timeline,
        Self::Render,
        Self::Cloth,
        Self::SetCloth,
        Self::Wind,
        Self::SetWind,
    ];
}
