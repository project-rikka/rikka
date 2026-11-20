mod camera;
mod cloth;
mod error;
mod figure;
mod keys;
mod look;
mod pose;
mod render;
mod scene;
mod timeline;
mod wind;

use animate_assistant::{ToolRequest, number};
use rikka_model::glam::Vec3;
use rikka_tool::Reply;
use serde_json::Value as Json;
use tokio::sync::oneshot;

pub(crate) use error::ServeError;

use animate_cast::FigureId;

use crate::animate::Animate;

pub(crate) type ToolResult = Result<Reply, ServeError>;
pub(crate) type ReplySender = oneshot::Sender<ToolResult>;

impl Animate {
    pub(crate) fn handle_tool_request(&mut self, request: ToolRequest, reply: ReplySender) {
        self.in_undo_group(move |animate| {
            let result = match request {
                ToolRequest::Scene => Ok(animate.describe_scene()),
                ToolRequest::Figure { figure, frame } => animate.read_figure(&figure, frame),
                ToolRequest::Pose { figure, keys } => animate.pose_keys(&figure, keys),
                ToolRequest::Look(query) => animate.read_look(&query),
                ToolRequest::Nodes(query) => animate.read_nodes(&query),
                ToolRequest::EditGraph(edits) => animate.edit_graph(&edits),
                ToolRequest::Materials(edits) => animate.set_materials(&edits),
                ToolRequest::LookKeys(keys) => animate.key_look(&keys),
                ToolRequest::ResetLook(reset) => animate.reset_look_graphs(reset),
                ToolRequest::Camera { keys } => Ok(animate.key_camera(&keys)),
                ToolRequest::Keys(request) => animate.work_keys(&request),
                ToolRequest::Timeline { frame, play } => Ok(animate.move_timeline(frame, play)),
                ToolRequest::Cloth { figure } => animate.read_cloth(&figure),
                ToolRequest::SetCloth {
                    figure,
                    feel,
                    pieces,
                } => animate.set_cloth(&figure, &feel, &pieces),
                ToolRequest::Wind { frame } => Ok(animate.read_wind(frame)),
                ToolRequest::SetWind(edit) => Ok(animate.set_wind(&edit)),
                ToolRequest::Render(request) => return animate.render(request, reply),
            };
            let _ = reply.send(result);
        });
    }

    fn handles(&self) -> Vec<(String, FigureId)> {
        let figures = self.cast.figures();
        figures
            .iter()
            .enumerate()
            .map(|(at, figure)| {
                let before = figures[..at]
                    .iter()
                    .filter(|earlier| earlier.name == figure.name)
                    .count();
                let handle = match before {
                    0 => figure.name.clone(),
                    more => format!("{} ({})", figure.name, more + 1),
                };
                (handle, figure.id)
            })
            .collect()
    }

    fn figure_called(&self, name: &str) -> Result<FigureId, ServeError> {
        let handles = self.handles();
        if let Some((_, id)) = handles.iter().find(|(handle, _)| handle == name) {
            return Ok(*id);
        }
        match handles.is_empty() {
            true => Err(ServeError::NoFigures),
            false => Err(ServeError::UnknownFigure {
                name: name.to_owned(),
                figures: handles
                    .iter()
                    .map(|(handle, _)| format!("\"{handle}\""))
                    .collect::<Vec<String>>()
                    .join(", "),
            }),
        }
    }

    fn frame_or_now(&self, frame: Option<u32>) -> u32 {
        frame.unwrap_or_else(|| self.transport.frame())
    }
}

pub(super) fn round(value: f32) -> Json {
    number(value).parse().map_or(Json::Null, Json::Number)
}

pub(super) fn rounded(point: Vec3) -> Json {
    Json::Array(point.to_array().into_iter().map(round).collect())
}
