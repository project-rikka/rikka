use std::sync::Arc;

use rikka_agent::{
    Agent, Budget, Cache, CancelToken, Content, Effort, Error as AgentError, Format, Image, Model,
    Schema,
};
use rikka_llm::ModelInfo;
use rikka_llm_accounts::{Error as AccountError, Llm, Selection};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::language::Language;
use crate::profile::Profile;
use crate::prompts::Prompts;

const ANSWER: &str = "Translation";

const SOURCE: &str = "The BCP-47 tag of the language the material turned out to be written in";

const ORIGINAL: &str = "The text as it stands in the picture";

const TRANSLATION: &str = "The material in the language it was asked for";

#[derive(Clone, Debug)]
pub(crate) enum Subject {
    Text(String),
    Picture(Arc<[u8]>),
}

impl Subject {
    #[must_use]
    fn is_picture(&self) -> bool {
        matches!(self, Subject::Picture(_))
    }

    #[must_use]
    fn content(&self) -> Vec<Content> {
        match self {
            Subject::Text(text) => vec![Content::text(text.clone())],
            Subject::Picture(png) => vec![Content::Image(Image::png(png.to_vec()))],
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Rendered {
    pub(crate) source: Language,
    pub(crate) original: Option<String>,
    pub(crate) translation: String,
}

pub(super) struct TranslationRequest<'a> {
    pub(crate) llm: &'a Llm,
    pub(crate) prompts: &'a Prompts,
    pub(crate) chosen: Option<&'a Selection>,
    pub(crate) target: &'a Language,
    pub(crate) source: &'a Language,
    pub(crate) profile: Profile,
    pub(crate) subject: Subject,
    pub(crate) cancel: CancelToken,
}

#[derive(Debug)]
pub(crate) struct Job {
    agent: Agent,
    subject: Subject,
}

impl Job {
    pub(super) fn prepare(request: TranslationRequest<'_>) -> Result<Job> {
        if let Subject::Text(text) = &request.subject {
            let most = request.prompts.max_characters()?;
            let length = text.chars().count();
            if length > most {
                return Err(Error::TooMuch { length, most });
            }
        }

        let picture = request.subject.is_picture();
        let model = model(&request)?;
        let instructions = match picture {
            true => request.prompts.image(request.target, request.source)?,
            false => request
                .prompts
                .text(request.target, request.source, request.profile)?,
        };

        let effort = effort(&request);
        let agent = Agent::new(model)
            .instructions(instructions)
            .format(Format::new(ANSWER, shape(picture)))
            .budget(Budget::new().turns(1))
            .cache(Cache::Off)
            .cancel_token(request.cancel);
        let agent = match effort {
            Some(effort) => agent.effort(effort),
            None => agent,
        };

        Ok(Job {
            agent: agent.build().map_err(failed)?,
            subject: request.subject,
        })
    }

    pub(crate) async fn run(self) -> Result<Rendered> {
        let outcome = self
            .agent
            .run(self.subject.content())
            .await
            .map_err(failed)?;
        let answer = answered(outcome.text())?;
        Ok(Rendered {
            source: Language::read(&answer.source),
            original: self.subject.is_picture().then_some(answer.original),
            translation: answer.translation,
        })
    }
}

fn model(request: &TranslationRequest<'_>) -> Result<Model> {
    if let (true, Some(info)) = (request.subject.is_picture(), about(request))
        && !info.vision()
    {
        return Err(Error::Blind {
            model: info.name().to_owned(),
        });
    }
    match request.chosen {
        Some(chosen) => chosen.connect(request.llm),
        None => request.llm.connect(),
    }
    .map_err(refused)
}

fn about<'a>(request: &TranslationRequest<'a>) -> Option<&'a ModelInfo> {
    match request.chosen {
        Some(chosen) => chosen.model_info(request.llm),
        None => request.llm.selected_account()?.model_info(),
    }
}

fn effort(request: &TranslationRequest<'_>) -> Option<Effort> {
    let preferred = request
        .chosen
        .map_or_else(|| request.llm.effort(), Selection::effort);
    match about(request) {
        Some(info) => info.nearest_effort(preferred),
        None => Some(preferred),
    }
}

fn shape(picture: bool) -> Schema {
    let answer = Schema::object()
        .param("source", Schema::string().describe(SOURCE))
        .param("translation", Schema::string().describe(TRANSLATION));
    match picture {
        true => answer.param("original", Schema::string().describe(ORIGINAL)),
        false => answer,
    }
}

#[derive(Debug, Default, Deserialize)]
struct ModelResponse {
    #[serde(default)]
    source: String,
    #[serde(default)]
    original: String,
    translation: String,
}

fn answered(text: &str) -> Result<ModelResponse> {
    let malformed = |error: serde_json::Error| Error::Malformed(error.to_string());
    match serde_json::from_str::<ModelResponse>(text.trim()) {
        Ok(answer) => Ok(answer),
        Err(error) => {
            serde_json::from_str(object(text).ok_or_else(|| malformed(error))?).map_err(malformed)
        }
    }
}

fn object(text: &str) -> Option<&str> {
    let from = text.find('{')?;
    let to = text.rfind('}')?;
    (from < to).then(|| &text[from..=to])
}

fn refused(error: AccountError) -> Error {
    match error {
        AccountError::Unconfigured => Error::Unconfigured,
        refused => Error::Refused(refused.to_string()),
    }
}

fn failed(error: AgentError) -> Error {
    Error::Refused(error.to_string())
}
