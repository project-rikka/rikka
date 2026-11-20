use rikka_net::HeaderMap;
use serde_json::{Map, Value, json};

use crate::effort::Effort;
use crate::error::{Error, Result};
use crate::listing::{ListedModel, output_tokens, text, tokens};
use crate::request::Request;
use crate::transport::Transport;

pub async fn models(transport: &Transport, project: &str) -> Result<Vec<ListedModel>> {
    let answer = transport
        .post(
            &json!({ "project": project }),
            &Request::default(),
            &HeaderMap::new(),
        )
        .await?;
    let models = answer
        .get("models")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::Malformed("the model listing holds no models".to_owned()))?;

    let mut folded: Vec<Folded> = Vec::new();
    for id in agent_lineup(&answer, models) {
        let Some(model) = models.get(&id) else {
            continue;
        };
        let (base, level) = leveled(&id, model);
        match folded.iter_mut().find(|held| held.base == base) {
            Some(held) => held.levels.extend(level),
            None => folded.push(Folded {
                base,
                model,
                levels: level.into_iter().collect(),
                leveled: level.is_some(),
            }),
        }
    }
    Ok(folded.into_iter().map(Folded::listed).collect())
}

struct Folded<'a> {
    base: String,
    model: &'a Value,
    levels: Vec<Effort>,
    leveled: bool,
}

impl Folded<'_> {
    fn listed(self) -> ListedModel {
        let name = text(self.model, "displayName").map(|name| match self.leveled {
            true => unlevelled(&name).to_owned(),
            false => name,
        });
        let thinks = self.model.get("supportsThinking").and_then(Value::as_bool) == Some(true);
        ListedModel::new(self.base)
            .with_name(name)
            .with_context(tokens(self.model, "maxTokens"))
            .with_output(output_tokens(self.model, "maxOutputTokens"))
            .with_vision(self.model.get("supportsImages").and_then(Value::as_bool))
            .with_efforts(Some(match thinks {
                true => self.levels,
                false => Vec::new(),
            }))
    }
}

fn agent_lineup(answer: &Value, models: &Map<String, Value>) -> Vec<String> {
    let sorted: Vec<String> = answer
        .get("agentModelSorts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|sort| sort.get("groups").and_then(Value::as_array))
        .flatten()
        .filter_map(|group| group.get("modelIds").and_then(Value::as_array))
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    if !sorted.is_empty() {
        return sorted;
    }
    models
        .iter()
        .filter(|(_, model)| {
            text(model, "displayName").is_some()
                && model.get("isInternal").and_then(Value::as_bool) != Some(true)
        })
        .map(|(id, _)| id.clone())
        .collect()
}

fn leveled(id: &str, model: &Value) -> (String, Option<Effort>) {
    let level = model
        .get("thinkingLevel")
        .filter(|level| level.is_number())
        .and_then(|_| id.rsplit_once('-'))
        .and_then(|(base, suffix)| {
            Effort::from_id(suffix)
                .filter(|effort| *effort != Effort::Off)
                .map(|effort| (base.to_owned(), effort))
        });
    match level {
        Some((base, effort)) => (base, Some(effort)),
        None => (id.to_owned(), None),
    }
}

fn unlevelled(name: &str) -> &str {
    match name.rsplit_once(" (") {
        Some((base, level)) if level.ends_with(')') => base,
        _ => name,
    }
}
