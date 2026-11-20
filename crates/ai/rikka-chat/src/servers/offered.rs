use rikka_mcp::ServerStatus;

use crate::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Offered {
    Tools,
    Prompts,
    Resources,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Listed {
    pub(crate) name: String,
    pub(crate) about: Option<String>,
}

impl Offered {
    pub(crate) const ALL: [Offered; 3] = [Offered::Tools, Offered::Prompts, Offered::Resources];

    pub(crate) fn count(self, standing: &ServerStatus) -> usize {
        match self {
            Offered::Tools => standing.tool_count(),
            Offered::Prompts => standing.prompt_count(),
            Offered::Resources => standing.resource_count(),
        }
    }

    pub(crate) fn listed(self, standing: &ServerStatus) -> Vec<Listed> {
        match self {
            Offered::Tools => standing
                .tools()
                .into_iter()
                .map(|tool| Listed {
                    name: tool.name().to_owned(),
                    about: tool.description().map(str::to_owned),
                })
                .collect(),
            Offered::Prompts => standing
                .prompts()
                .iter()
                .map(|prompt| Listed {
                    name: format!("/{}", prompt.command()),
                    about: prompt.description().or(prompt.title()).map(str::to_owned),
                })
                .collect(),
            Offered::Resources => {
                let resources = standing.resources();
                let templates = standing.resource_templates();
                resources
                    .iter()
                    .map(|resource| Listed {
                        name: format!("@{}", resource.mention()),
                        about: resource
                            .description()
                            .or(resource.title())
                            .map(str::to_owned),
                    })
                    .chain(templates.iter().map(|template| {
                        Listed {
                            name: format!("@{}", template.mention_template()),
                            about: template
                                .description()
                                .or(template.title())
                                .map(str::to_owned),
                        }
                    }))
                    .collect()
            }
        }
    }

    #[expect(
        clippy::cast_possible_wrap,
        reason = "a count of offers, which no server makes more of than an i64 holds"
    )]
    pub(crate) fn counted(self, count: usize) -> Label {
        let count = count as i64;
        match self {
            Offered::Tools => tr!("chat.servers.tools", count = count),
            Offered::Prompts => tr!("chat.servers.prompts", count = count),
            Offered::Resources => tr!("chat.servers.resources", count = count),
        }
    }
}
