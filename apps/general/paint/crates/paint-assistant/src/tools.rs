use std::sync::Arc;

use rikka_assist::Prepared;
use rikka_prompt::{Facts, Prompt};
use rikka_tool::Function;
use rikka_tool::{Schema, ToolBuilder, Value};

use crate::error::{Error, Result};
use crate::ops::{self, Parsed};
use crate::order::Order;
use crate::parse;
use crate::tool::Tool;
use crate::tool_host::ToolHost;
use crate::vocabulary::{
    ALIGNS, ANCHORS, BLENDS, CAPS, COMBINES, FILL_RULES, FILLINGS, FLIPS, FLOODS, Fields,
    GRADIENTS, HISTORY, INTENTS, ITEMS, JOINS, LAYERINGS, OBJECTINGS, RESAMPLINGS, SAMPLES,
    SELECTIONS, STACKINGS, TRANSFORMS, TURNS, names,
};

type Reading = Arc<dyn Fn(Value) -> Parsed<Order> + Send + Sync>;

struct ToolDescriber<'a> {
    prompt: &'a Prompt,
    facts: &'a Facts,
}

impl ToolDescriber<'_> {
    fn description(&self, name: &str) -> Result<String> {
        Ok(self.prompt.param(name, self.facts)?)
    }

    fn describe(&self, name: &str, schema: Schema) -> Result<Schema> {
        Ok(schema.describe(self.description(name)?))
    }

    fn string(&self, name: &str) -> Result<Schema> {
        self.describe(name, Schema::string())
    }

    fn integer(&self, name: &str) -> Result<Schema> {
        self.describe(name, Schema::integer())
    }

    fn boolean(&self, name: &str) -> Result<Schema> {
        self.describe(name, Schema::boolean())
    }

    fn rect(&self, name: &str) -> Result<Schema> {
        self.describe(name, numbers())
    }

    fn document(&self, tool: ToolBuilder) -> Result<ToolBuilder> {
        Ok(tool
            .param("document", self.string("document")?)
            .optional("document"))
    }

    fn named(&self, tool: ToolBuilder) -> Result<ToolBuilder> {
        Ok(self.document(tool)?.param("name", self.string("name")?))
    }
}

fn numbers() -> Schema {
    Schema::array(Schema::number())
}

fn choice(names: impl IntoIterator<Item = &'static str>) -> Schema {
    Schema::string().enumerate(names)
}

fn layer() -> Schema {
    Schema::any_of([Schema::integer(), Schema::string()])
}

fn item(key: &str, table: &[(&'static str, Fields)]) -> Schema {
    let mut fields: Vec<&'static str> = table
        .iter()
        .flat_map(|(_, fields)| fields.iter().copied())
        .collect();
    fields.sort_unstable();
    fields.dedup();
    fields.into_iter().fold(
        Schema::object().param(key, choice(names(table))),
        |item, name| match field(name) {
            Some(schema) => item.optional(name, schema),
            None => item,
        },
    )
}

pub(crate) fn field(name: &str) -> Option<Schema> {
    Some(match name {
        "points" => Schema::array(numbers()),
        "rect" | "inside" | "at" | "from" | "to" | "fade" | "offset" | "around" => numbers(),
        "d" | "font" | "text" | "name" | "color" | "fill" | "stroke" => Schema::string(),
        "width" | "size" | "hardness" | "opacity" | "flow" | "tolerance" | "by" | "angle"
        | "degrees" => Schema::number(),
        "weight" | "wrap" | "dx" | "dy" | "object" => Schema::integer(),
        "objects" => Schema::array(Schema::integer()),
        "stack" => choice(names(&STACKINGS)),
        "item" => item("tool", &ITEMS),
        "italic" | "antialias" | "visible" => Schema::boolean(),
        "filling" => choice(FILLINGS),
        "fill_rule" => choice(names(&FILL_RULES)),
        "join" => choice(names(&JOINS)),
        "cap" => choice(names(&CAPS)),
        "shape" => choice(names(&GRADIENTS)),
        "colors" => Schema::array(Schema::string()),
        "align" => choice(names(&ALIGNS)),
        "anchor" => choice(names(&ANCHORS)),
        "mode" => choice(names(&FLOODS)),
        "sample" => choice(names(&SAMPLES)),
        "combine" => choice(names(&COMBINES)),
        "blend" => choice(names(&BLENDS)),
        "layer" | "above" | "below" | "into" => layer(),
        _ => return None,
    })
}

fn reading(tool: Tool) -> Reading {
    match tool {
        Tool::Document => Arc::new(parse::document),
        Tool::View => Arc::new(parse::view),
        Tool::Measure => Arc::new(parse::measure),
        Tool::Adjust => Arc::new(parse::adjust),
        Tool::Draw => Arc::new(parse::draw),
        Tool::Select => Arc::new(parse::select),
        Tool::Layers => Arc::new(parse::layers),
        Tool::Transform => Arc::new(parse::transform),
        Tool::Objects => Arc::new(parse::objects),
        Tool::History => Arc::new(parse::history),
        Tool::Save => Arc::new(|input| parse::save(input, rikka_dirs::home())),
        Tool::Open => Arc::new(parse::open),
        Tool::New => Arc::new(parse::new),
    }
}

pub fn order(tool: &str, input: Value) -> std::result::Result<Order, String> {
    let known = Tool::ALL
        .into_iter()
        .find(|known| known.stem() == tool)
        .ok_or_else(|| {
            format!(
                "Paint has no tool \"{tool}\"; its tools are {}",
                Tool::ALL.map(Tool::stem).join(", ")
            )
        })?;
    reading(known)(input)
}

pub(crate) fn declare(prepared: &Prepared, host: &Arc<dyn ToolHost>) -> Result<Vec<Function>> {
    let facts = prepared.facts();
    Tool::ALL
        .into_iter()
        .map(|tool| {
            let prompt = prepared.tool(tool as usize);
            let declared = schema(
                tool,
                rikka_tool::Tool::new(prompt.name()).description(prompt.body(facts)?),
                &ToolDescriber { prompt, facts },
            )?
            .build()
            .map_err(|source| Error::Declaration {
                name: prompt.name().to_owned(),
                source,
            })?;
            Ok(function(declared, reading(tool), Arc::clone(host)))
        })
        .collect()
}

fn function(tool: rikka_tool::Tool, reading: Reading, host: Arc<dyn ToolHost>) -> Function {
    Function::new(tool, move |input: Value| {
        let order = reading(input);
        let host = Arc::clone(&host);
        async move { host.handle(order?).await }
    })
}

fn schema(kind: Tool, tool: ToolBuilder, descriptions: &ToolDescriber<'_>) -> Result<ToolBuilder> {
    Ok(match kind {
        Tool::Document => descriptions.document(tool)?,
        Tool::View => descriptions
            .document(tool)?
            .param("region", descriptions.rect("region")?)
            .param("width", descriptions.integer("width")?)
            .param("layer", descriptions.describe("layer", layer())?)
            .param(
                "grid",
                descriptions.describe(
                    "grid",
                    Schema::any_of([Schema::boolean(), Schema::integer()]),
                )?,
            )
            .param("selection", descriptions.boolean("selection")?)
            .param("compare", descriptions.boolean("compare")?)
            .optional("region")
            .optional("width")
            .optional("layer")
            .optional("grid")
            .optional("selection")
            .optional("compare"),
        Tool::Measure => descriptions
            .document(tool)?
            .param(
                "area",
                descriptions.describe(
                    "area",
                    Schema::any_of([choice(["selection", "whole"]), numbers()]),
                )?,
            )
            .param("layer", descriptions.describe("layer", layer())?)
            .param(
                "points",
                descriptions.describe("points", Schema::array(numbers()))?,
            )
            .param("radius", descriptions.integer("radius")?)
            .optional("area")
            .optional("layer")
            .optional("points")
            .optional("radius"),
        Tool::Adjust => {
            let mask = Schema::object()
                .optional("rect", numbers())
                .optional("ellipse", numbers())
                .optional("path", Schema::string())
                .optional("fill_rule", choice(names(&FILL_RULES)))
                .optional("feather", Schema::number())
                .optional("invert", Schema::boolean());
            let step = Schema::object()
                .param("op", choice(ops::names()))
                .optional("layer", layer())
                .optional("mask", mask);
            descriptions.named(tool)?.param(
                "steps",
                descriptions.describe("steps", Schema::array(step))?,
            )
        }
        Tool::Draw => descriptions
            .named(tool)?
            .param("layer", descriptions.describe("layer", layer())?)
            .param(
                "items",
                descriptions.describe("items", Schema::array(item("tool", &ITEMS)))?,
            )
            .optional("layer"),
        Tool::Select => descriptions.named(tool)?.param(
            "steps",
            descriptions.describe("steps", Schema::array(item("op", &SELECTIONS)))?,
        ),
        Tool::Objects => descriptions.named(tool)?.param(
            "steps",
            descriptions.describe("steps", Schema::array(item("op", &OBJECTINGS)))?,
        ),
        Tool::Layers => descriptions.named(tool)?.param(
            "steps",
            descriptions.describe("steps", Schema::array(item("op", &LAYERINGS)))?,
        ),
        Tool::Transform => {
            let step = Schema::object()
                .param("op", choice(TRANSFORMS))
                .optional("rect", numbers())
                .optional("width", Schema::integer())
                .optional("height", Schema::integer())
                .optional("resampling", choice(names(&RESAMPLINGS)))
                .optional("anchor", choice(names(&ANCHORS)))
                .optional("fill", Schema::string())
                .optional("degrees", Schema::number())
                .optional("direction", choice(names(&TURNS)))
                .optional("axis", choice(names(&FLIPS)))
                .optional("profile", Schema::string())
                .optional("intent", choice(names(&INTENTS)));
            descriptions.named(tool)?.param(
                "steps",
                descriptions.describe("steps", Schema::array(step))?,
            )
        }
        Tool::History => descriptions
            .document(tool)?
            .param("action", descriptions.describe("action", choice(HISTORY))?)
            .param("steps", descriptions.integer("steps")?)
            .param("index", descriptions.integer("index")?)
            .optional("steps")
            .optional("index"),
        Tool::Save => descriptions
            .document(tool)?
            .param("path", descriptions.string("path")?)
            .param("quality", descriptions.integer("quality")?)
            .optional("path")
            .optional("quality"),
        Tool::Open => tool.param("path", descriptions.string("path")?),
        Tool::New => tool
            .param("width", descriptions.integer("width")?)
            .param("height", descriptions.integer("height")?)
            .param(
                "resolution",
                descriptions.describe("resolution", Schema::number())?,
            )
            .param("background", descriptions.string("background")?)
            .optional("resolution")
            .optional("background"),
    })
}
