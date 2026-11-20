use rikka_nixos_layout::Module;

use crate::{
    FLAKE, answers::Answers, configuration, error::Result, flake, password::hash_password,
    rikka::RenderModule,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratedFile {
    pub path: String,
    pub contents: String,
    pub mode: u32,
    pub secret: bool,
}

pub const SECRETS_MODE: u32 = 0o700;

pub fn generate(answers: &Answers) -> Result<Vec<GeneratedFile>> {
    let mut files = vec![
        GeneratedFile {
            path: format!("{FLAKE}/flake.nix"),
            contents: flake::render(answers)?,
            mode: 0o644,
            secret: false,
        },
        GeneratedFile {
            path: format!("{FLAKE}/configuration.nix"),
            contents: configuration::render(answers)?,
            mode: 0o644,
            secret: false,
        },
    ];

    for module in Module::EVERY {
        files.push(GeneratedFile {
            path: module.path(),
            contents: module.render(answers)?,
            mode: 0o644,
            secret: false,
        });
    }

    files.push(GeneratedFile {
        path: answers.password_file(),
        contents: format!("{}\n", hash_password(&answers.password)?),
        mode: 0o400,
        secret: true,
    });

    if answers.set_root_password {
        files.push(GeneratedFile {
            path: Answers::root_password_file(),
            contents: format!("{}\n", hash_password(&answers.root_password)?),
            mode: 0o400,
            secret: true,
        });
    }

    Ok(files)
}
