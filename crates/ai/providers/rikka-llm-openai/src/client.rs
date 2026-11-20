use rikka_llm::wire::{Responses, ResponsesDialect, Transport};
use rikka_llm::{Model, Result};
use rikka_llm_adapter::{Api, Keyed, KeyedConnection};

pub type OpenAi = Keyed<OpenAiApi>;

#[must_use]
#[derive(Debug, Default)]
pub struct OpenAiApi {
    organization: Option<String>,
    project: Option<String>,
}

impl OpenAiApi {
    pub fn organization(mut self, organization: impl Into<String>) -> OpenAiApi {
        self.organization = Some(organization.into());
        self
    }

    pub fn project(mut self, project: impl Into<String>) -> OpenAiApi {
        self.project = Some(project.into());
        self
    }
}

impl Api for OpenAiApi {
    const VARIABLE: &'static str = "OPENAI_API_KEY";

    const ENDPOINT: &'static str = "https://api.openai.com/v1/responses";

    fn connect(self, connection: KeyedConnection) -> Result<Model> {
        let KeyedConnection {
            model,
            key,
            endpoint,
            headers,
        } = connection;

        let mut headers = headers.bearer(&key);
        if let Some(organization) = &self.organization {
            headers = headers.text("openai-organization", organization, "the organization");
        }
        if let Some(project) = &self.project {
            headers = headers.text("openai-project", project, "the project");
        }

        Ok(Responses::new(
            Transport::new(endpoint, headers.json().build()?)?,
            model,
            ResponsesDialect::OpenAi,
        )
        .build())
    }
}
