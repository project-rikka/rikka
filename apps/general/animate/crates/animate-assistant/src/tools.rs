use std::sync::Arc;

use animate_motion::GazeMode;
use rikka_assist::Prepared;
use rikka_prompt::{Facts, Prompt};
use rikka_tool::Function;
use rikka_tool::{Schema, ToolBuilder, Value};
use serde_json::json;

use crate::error::{Error, Result};
use crate::parse::{self, ACTIONS, FILMS, Parsed};
use crate::request::ToolRequest;
use crate::tool::Tool;
use crate::tool_error::ToolError;
use crate::tool_host::ToolHost;
use crate::{cloth, look, wind};

type RequestParser = Arc<dyn Fn(Value) -> Parsed<ToolRequest> + Send + Sync>;

struct ParamDocs<'a> {
    prompt: &'a Prompt,
    facts: &'a Facts,
}

impl ParamDocs<'_> {
    fn description(&self, name: &str) -> Result<String> {
        Ok(self.prompt.param(name, self.facts)?)
    }

    fn string(&self, name: &str) -> Result<Schema> {
        Ok(Schema::string().describe(self.description(name)?))
    }

    fn integer(&self, name: &str) -> Result<Schema> {
        Ok(Schema::integer().describe(self.description(name)?))
    }

    fn number(&self, name: &str) -> Result<Schema> {
        Ok(Schema::number().describe(self.description(name)?))
    }

    fn any(&self, name: &str) -> Result<Schema> {
        Ok(Schema::any().describe(self.description(name)?))
    }

    fn boolean(&self, name: &str) -> Result<Schema> {
        Ok(Schema::boolean().describe(self.description(name)?))
    }

    fn point(&self, name: &str) -> Result<Schema> {
        Ok(point().describe(self.description(name)?))
    }

    fn strings(&self, name: &str) -> Result<Schema> {
        Ok(Schema::array(Schema::string()).describe(self.description(name)?))
    }

    fn map(&self, name: &str, of: Schema) -> Result<Schema> {
        Ok(Schema::raw(json!({
            "type": "object",
            "additionalProperties": of.into_value(),
            "description": self.description(name)?,
        })))
    }

    fn curve(&self, name: &str) -> Result<Schema> {
        Ok(
            Schema::any_of([Schema::string(), Schema::array(Schema::integer())])
                .describe(self.description(name)?),
        )
    }

    fn placement(&self, keyed: bool) -> Result<Schema> {
        let mut placed = Schema::object();
        if keyed {
            placed = placed.param("frame", self.integer("frame")?);
        }
        placed = placed
            .param("target", self.point("target")?)
            .optional("eye", self.point("eye")?)
            .optional("yaw", self.number("yaw")?)
            .optional("elevation", self.number("elevation")?)
            .optional("distance", self.number("distance")?)
            .optional("roll", self.number("roll")?)
            .optional("fov", self.number("fov")?);
        if keyed {
            placed = placed.optional("curve", self.curve("curve")?);
        }
        Ok(placed)
    }
}

fn point() -> Schema {
    Schema::array(Schema::number())
}

pub(crate) fn declare(prepared: &Prepared, tool_host: &Arc<dyn ToolHost>) -> Result<Vec<Function>> {
    let facts = prepared.facts();
    let render = prepared.tool(Tool::Render as usize);
    let camera = prepared.tool(Tool::Camera as usize);
    let most_frames = usize::try_from(render.limit("most_frames")?).unwrap_or(usize::MAX);
    let widest = u32::try_from(render.limit("widest")?).unwrap_or(u32::MAX);
    let reading = |tool: Tool| -> RequestParser {
        match tool {
            Tool::Scene => Arc::new(parse::scene),
            Tool::Figure => Arc::new(parse::figure),
            Tool::Pose => Arc::new(parse::pose),
            Tool::Look => Arc::new(look::look),
            Tool::Nodes => Arc::new(look::nodes),
            Tool::EditGraph => Arc::new(look::edit_graph),
            Tool::Materials => Arc::new(look::materials),
            Tool::LookKeys => Arc::new(look::look_keys),
            Tool::ResetLook => Arc::new(look::look_reset),
            Tool::Camera => Arc::new(parse::camera),
            Tool::Keys => Arc::new(parse::keys),
            Tool::Timeline => Arc::new(parse::timeline),
            Tool::Render => Arc::new(move |input| parse::render(input, most_frames, widest)),
            Tool::Cloth => Arc::new(cloth::cloth),
            Tool::SetCloth => Arc::new(cloth::set_cloth),
            Tool::Wind => Arc::new(wind::wind),
            Tool::SetWind => Arc::new(wind::set_wind),
        }
    };
    Tool::ALL
        .into_iter()
        .map(|tool| {
            let prompt = prepared.tool(tool as usize);
            let declared = schema(
                tool,
                rikka_tool::Tool::new(prompt.name()).description(prompt.body(facts)?),
                &ParamDocs { prompt, facts },
                &ParamDocs {
                    prompt: camera,
                    facts,
                },
            )?
            .build()
            .map_err(|source| Error::Declaration {
                name: prompt.name().to_owned(),
                source,
            })?;
            Ok(function(declared, reading(tool), Arc::clone(tool_host)))
        })
        .collect()
}

fn function(
    tool: rikka_tool::Tool,
    reading: RequestParser,
    tool_host: Arc<dyn ToolHost>,
) -> Function {
    Function::new(tool, move |input: Value| {
        let request = reading(input).map_err(ToolError::from);
        let tool_host = Arc::clone(&tool_host);
        async move { tool_host.handle(request?).await }
    })
}

fn schema(
    kind: Tool,
    tool: ToolBuilder,
    params: &ParamDocs<'_>,
    placing: &ParamDocs<'_>,
) -> Result<ToolBuilder> {
    Ok(match kind {
        Tool::Scene => tool,
        Tool::Figure => tool
            .param("figure", params.string("figure")?)
            .param("frame", params.integer("frame")?)
            .optional("frame"),
        Tool::Pose => {
            let angles = Schema::raw(json!({
                "type": "object",
                "additionalProperties": { "type": "number" },
            }));
            let hips = Schema::object()
                .optional("position", point())
                .optional("yaw", Schema::number())
                .optional("pitch", Schema::number())
                .optional("roll", Schema::number())
                .describe(params.description("hips")?);
            let gaze = Schema::object()
                .optional(
                    "mode",
                    Schema::string().enumerate(GazeMode::ALL.map(GazeMode::name)),
                )
                .optional("target", point())
                .optional("weight", Schema::number())
                .optional("headFollow", Schema::number())
                .optional(
                    "eyes",
                    Schema::raw(json!({
                        "type": "object",
                        "additionalProperties": {
                            "type": "array",
                            "items": { "type": "number" },
                            "minItems": 2,
                            "maxItems": 2,
                        },
                    })),
                )
                .describe(params.description("gaze")?);
            let key = Schema::object()
                .param("frame", params.integer("frame")?)
                .optional("tag", params.string("tag")?)
                .optional("rest", params.strings("rest")?)
                .optional("joints", params.map("joints", angles)?)
                .optional("hips", hips)
                .optional("hands", params.map("hands", Schema::string())?)
                .optional("expressions", params.map("expressions", Schema::number())?)
                .optional("morphs", params.map("morphs", Schema::number())?)
                .optional("gaze", gaze)
                .optional("reach", params.map("reach", point())?)
                .optional("pins", params.map("pins", Schema::boolean())?);
            tool.param("figure", params.string("figure")?).param(
                "keys",
                Schema::array(key).describe(params.description("keys")?),
            )
        }
        Tool::Look => tool
            .param("graph", params.string("graph")?)
            .param("figure", params.string("figure")?)
            .optional("graph")
            .optional("figure"),
        Tool::Nodes => tool
            .param(
                "graph",
                Schema::string()
                    .enumerate(["world", "composite", "material"])
                    .describe(params.description("graph")?),
            )
            .param("search", params.string("search")?)
            .optional("search"),
        Tool::EditGraph => tool.param("graph", params.string("graph")?).param(
            "edits",
            Schema::array(Schema::object().describe(params.description("edit")?))
                .describe(params.description("edits")?),
        ),
        Tool::Materials => {
            let add = Schema::object()
                .param("name", params.string("name")?)
                .optional("preset", params.string("preset")?);
            let rename = Schema::object()
                .param("from", params.string("from")?)
                .param("to", params.string("to")?);
            let assign = Schema::object()
                .param("figure", params.string("figure")?)
                .param("slots", params.any("slots")?)
                .param("material", params.any("material")?)
                .optional("unique", params.boolean("unique")?);
            let value = Schema::object()
                .param("figure", params.string("figure")?)
                .param("slot", params.any("slot")?)
                .param("parameter", params.string("parameter")?)
                .param("value", params.any("value")?);
            tool.param(
                "add",
                Schema::array(add).describe(params.description("add")?),
            )
            .param("remove", params.strings("remove")?)
            .param(
                "rename",
                Schema::array(rename).describe(params.description("rename")?),
            )
            .param(
                "assign",
                Schema::array(assign).describe(params.description("assign")?),
            )
            .param(
                "values",
                Schema::array(value).describe(params.description("values")?),
            )
            .optional("add")
            .optional("remove")
            .optional("rename")
            .optional("assign")
            .optional("values")
        }
        Tool::LookKeys => {
            let set = Schema::object()
                .param("target", params.any("target")?)
                .param("frame", params.integer("frame")?)
                .optional("value", params.any("value")?)
                .optional("curve", params.curve("curve")?);
            let remove = Schema::object()
                .param("target", params.any("target")?)
                .optional("frame", params.integer("frame")?);
            tool.param(
                "set",
                Schema::array(set).describe(params.description("set")?),
            )
            .param(
                "remove",
                Schema::array(remove).describe(params.description("remove")?),
            )
            .optional("set")
            .optional("remove")
        }
        Tool::ResetLook => {
            let starter = |name: &str| -> Result<Schema> {
                Ok(Schema::string()
                    .enumerate(["bare", "cinematic"])
                    .describe(params.description(name)?))
            };
            tool.param("world", starter("world")?)
                .param("composite", starter("composite")?)
                .optional("world")
                .optional("composite")
        }
        Tool::Camera => tool.param(
            "keys",
            Schema::array(params.placement(true)?).describe(params.description("keys")?),
        ),
        Tool::Keys => tool
            .param("subject", params.string("subject")?)
            .param(
                "action",
                Schema::string()
                    .enumerate(ACTIONS)
                    .describe(params.description("action")?),
            )
            .param(
                "lanes",
                Schema::array(Schema::any()).describe(params.description("lanes")?),
            )
            .param("from", params.integer("from")?)
            .param("to", params.integer("to")?)
            .param("by", params.integer("by")?)
            .param("curve", params.curve("curve")?)
            .param("tag", params.string("tag")?)
            .optional("lanes")
            .optional("from")
            .optional("to")
            .optional("by")
            .optional("curve")
            .optional("tag"),
        Tool::Timeline => tool
            .param("frame", params.integer("frame")?)
            .param("play", params.boolean("play")?)
            .optional("frame")
            .optional("play"),
        Tool::Render => tool
            .param(
                "frames",
                Schema::array(Schema::integer()).describe(params.description("frames")?),
            )
            .param(
                "camera",
                Schema::any_of([Schema::string().enumerate(FILMS), placing.placement(false)?])
                    .describe(params.description("camera")?),
            )
            .param("width", params.integer("width")?)
            .optional("frames")
            .optional("camera")
            .optional("width"),
        Tool::Cloth => tool.param("figure", params.string("figure")?),
        Tool::SetCloth => {
            let piece = Schema::object()
                .param("piece", params.string("piece")?)
                .optional("preset", params.string("preset")?)
                .optional("enabled", params.boolean("enabled")?)
                .optional("feel", params.map("feel", Schema::number())?)
                .optional("reset", params.boolean("reset")?);
            tool.param("figure", params.string("figure")?)
                .param("feel", params.map("whole", Schema::number())?)
                .param(
                    "pieces",
                    Schema::array(piece).describe(params.description("pieces")?),
                )
                .optional("feel")
                .optional("pieces")
        }
        Tool::Wind => tool
            .param("frame", params.integer("frame")?)
            .optional("frame"),
        Tool::SetWind => {
            let key = Schema::object()
                .param("frame", params.integer("frame")?)
                .param("values", params.map("values", Schema::number())?)
                .optional("curve", params.curve("curve")?);
            tool.param("resting", params.map("resting", Schema::number())?)
                .param(
                    "keys",
                    Schema::array(key).describe(params.description("keys")?),
                )
                .param("clear", params.strings("clear")?)
                .optional("resting")
                .optional("keys")
                .optional("clear")
        }
    })
}
