use ade_git::{Change, Patch, Repo};

pub(super) enum PatchRequest {
    File {
        repo: Repo,
        change: Change,
        born: bool,
    },
    Commit {
        repo: Repo,
        oid: String,
    },
}

impl PatchRequest {
    pub(super) fn read(self) -> Result<Patch, String> {
        match self {
            PatchRequest::File { repo, change, born } => repo.diff(&change, born),
            PatchRequest::Commit { repo, oid } => repo.patch(&oid),
        }
        .map_err(|error| error.to_string())
    }
}
