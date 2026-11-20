use rikka_llm::{Effort, Model, ModelInfo};

use crate::roster::ModelRoster;

#[derive(Clone, Debug)]
pub struct ConnectedModel {
    bench: ModelRoster,
    model: Model,
    info: Option<ModelInfo>,
    effort: Option<Effort>,
}

impl ConnectedModel {
    #[must_use]
    pub fn new(model: Model, info: Option<ModelInfo>, effort: Effort) -> ConnectedModel {
        ConnectedModel::from_bench(ModelRoster::none(), model, info, effort)
    }

    pub(crate) fn from_bench(
        bench: ModelRoster,
        model: Model,
        info: Option<ModelInfo>,
        requested_effort: Effort,
    ) -> ConnectedModel {
        let effort = info
            .as_ref()
            .and_then(|info| info.nearest_effort(requested_effort));
        ConnectedModel {
            bench,
            model,
            info,
            effort,
        }
    }

    #[must_use]
    pub fn model(&self) -> &Model {
        &self.model
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.info
            .as_ref()
            .map_or_else(|| self.model.name(), ModelInfo::name)
    }

    #[must_use]
    pub fn info(&self) -> Option<&ModelInfo> {
        self.info.as_ref()
    }

    #[must_use]
    pub fn context_window(&self) -> u64 {
        self.info.as_ref().map_or(0, ModelInfo::context)
    }

    #[must_use]
    pub fn max_output(&self) -> u64 {
        self.info
            .as_ref()
            .map_or(0, |info| u64::from(info.output()))
    }

    #[must_use]
    pub fn effort(&self) -> Option<Effort> {
        self.effort
    }

    #[must_use]
    pub fn roster(&self) -> &ModelRoster {
        &self.bench
    }

    #[must_use]
    pub fn with_effort(&self, effort: Effort) -> ConnectedModel {
        ConnectedModel::from_bench(
            self.bench.clone(),
            self.model.clone(),
            self.info.clone(),
            effort,
        )
    }

    #[must_use]
    pub fn label(&self) -> String {
        match self.effort.and_then(Effort::name) {
            Some(effort) => format!("{} ({effort})", self.name()),
            None => self.name().to_owned(),
        }
    }
}
