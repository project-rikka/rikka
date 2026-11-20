use std::path::PathBuf;

use rikka_agent::{Function, Reply};
use rikka_llm::{Tool, Value};
use rikka_tools::{Toolbox, ToolboxBuilder, facts};
use tempfile::TempDir;

pub(crate) struct Project {
    folder: TempDir,
    toolbox: Toolbox,
}

impl Project {
    pub(crate) fn open() -> Project {
        Project::fitted(|setup| setup)
    }

    pub(crate) fn fitted(fit: impl FnOnce(ToolboxBuilder) -> ToolboxBuilder) -> Project {
        let folder = tempfile::tempdir().expect("temp dir should be created");
        let toolbox = fit(Toolbox::builder(folder.path()))
            .build()
            .expect("tools should be declared");
        Project { folder, toolbox }
    }

    pub(crate) fn toolbox(&self) -> &Toolbox {
        &self.toolbox
    }

    pub(crate) fn at(&self, path: &str) -> PathBuf {
        self.folder.path().join(path)
    }

    pub(crate) fn holds(&self, path: &str, text: &str) {
        let at = self.at(path);
        if let Some(parent) = at.parent() {
            std::fs::create_dir_all(parent).expect("directory should be created");
        }
        std::fs::write(at, text).expect("file should be written");
    }

    pub(crate) fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.at(path)).expect("file should be readable")
    }

    pub(crate) fn declares(&self, stem: &str) -> bool {
        self.function(&named(stem)).is_some()
    }

    pub(crate) fn function(&self, name: &str) -> Option<&Function> {
        self.toolbox
            .functions()
            .iter()
            .find(|function| function.name() == name)
    }

    pub(crate) async fn call(&self, stem: &str, input: Value) -> Result<Reply, String> {
        self.function(&named(stem))
            .unwrap_or_else(|| panic!("{stem} is declared"))
            .call(input)
            .await
    }

    pub(crate) fn logged(&self) -> Vec<PathBuf> {
        let Ok(reading) = std::fs::read_dir(self.at(".rikka/tmp")) else {
            return Vec::new();
        };
        reading
            .flatten()
            .map(|entry| entry.path())
            .filter(|at| at.extension().is_some_and(|end| end == "log"))
            .collect()
    }

    pub(crate) fn only_log(&self) -> String {
        let logged = self.logged();
        assert_eq!(logged.len(), 1, "{logged:?}");
        std::fs::read_to_string(&logged[0]).expect("file should exist")
    }
}

pub(crate) fn named(stem: &str) -> String {
    facts()
        .expect("shipped documents should parse")
        .get(&format!("{stem}_tool"))
        .expect("every tool should have a name")
        .to_owned()
}

pub(crate) fn stranger(name: &str, description: &str) -> Function {
    let tool = Tool::from_schema(name, rikka_llm::json!({ "type": "object" }))
        .description(description)
        .build()
        .expect("tool should be valid");
    Function::new(tool, |_: Value| async {
        Ok::<Reply, String>(Reply::new("done"))
    })
}
