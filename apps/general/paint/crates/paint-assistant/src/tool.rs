use rikka_assist::KitTool;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Tool {
    Document,
    View,
    Measure,
    Adjust,
    Draw,
    Select,
    Layers,
    Transform,
    Objects,
    History,
    Save,
    Open,
    New,
}

pub(crate) static TOOLS: [KitTool; 13] = [
    KitTool {
        stem: "document",
        document: include_str!("../../../prompts/tools/document.md"),
        gist: &["document"],
    },
    KitTool {
        stem: "view",
        document: include_str!("../../../prompts/tools/view.md"),
        gist: &["document", "region"],
    },
    KitTool {
        stem: "measure",
        document: include_str!("../../../prompts/tools/measure.md"),
        gist: &["document"],
    },
    KitTool {
        stem: "adjust",
        document: include_str!("../../../prompts/tools/adjust.md"),
        gist: &["name", "steps[].op"],
    },
    KitTool {
        stem: "draw",
        document: include_str!("../../../prompts/tools/draw.md"),
        gist: &["name", "items[].tool"],
    },
    KitTool {
        stem: "select",
        document: include_str!("../../../prompts/tools/select.md"),
        gist: &["name", "steps[].op"],
    },
    KitTool {
        stem: "layers",
        document: include_str!("../../../prompts/tools/layers.md"),
        gist: &["name", "steps[].op"],
    },
    KitTool {
        stem: "transform",
        document: include_str!("../../../prompts/tools/transform.md"),
        gist: &["name", "steps[].op"],
    },
    KitTool {
        stem: "objects",
        document: include_str!("../../../prompts/tools/objects.md"),
        gist: &["name", "steps[].op"],
    },
    KitTool {
        stem: "history",
        document: include_str!("../../../prompts/tools/history.md"),
        gist: &["action"],
    },
    KitTool {
        stem: "save",
        document: include_str!("../../../prompts/tools/save.md"),
        gist: &["path"],
    },
    KitTool {
        stem: "open",
        document: include_str!("../../../prompts/tools/open.md"),
        gist: &["path"],
    },
    KitTool {
        stem: "new",
        document: include_str!("../../../prompts/tools/new.md"),
        gist: &["width", "height"],
    },
];

impl Tool {
    pub(crate) const ALL: [Self; 13] = [
        Self::Document,
        Self::View,
        Self::Measure,
        Self::Adjust,
        Self::Draw,
        Self::Select,
        Self::Layers,
        Self::Transform,
        Self::Objects,
        Self::History,
        Self::Save,
        Self::Open,
        Self::New,
    ];

    pub(crate) fn stem(self) -> &'static str {
        TOOLS[self as usize].stem
    }
}
