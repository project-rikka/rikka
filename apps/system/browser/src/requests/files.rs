use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_web::{FileMode, PickFiles};

use crate::shell::Shell;
use browser_model::TabId;

fn chooser(request: &PickFiles) -> Chooser {
    let title = match request.mode {
        FileMode::Folder => tr!("browser.files.choose-folder"),
        FileMode::Multiple => tr!("browser.files.choose-many"),
        FileMode::Single => tr!("browser.files.choose"),
    };
    let chooser = Chooser::open(title.to_string()).parent(window_identifier());
    if request.mode == FileMode::Folder {
        return chooser.directory(true);
    }
    let chooser = chooser.multiple(request.mode == FileMode::Multiple);
    let accepted: Vec<&String> = request
        .mime_types
        .iter()
        .filter(|mime| !mime.is_empty())
        .collect();
    if accepted.is_empty() {
        return chooser;
    }
    let filter = accepted.into_iter().fold(
        Filter::named(tr!("browser.files.accepted").to_string()),
        |filter, mime| filter.mime_type(mime.clone()),
    );
    chooser
        .filter(filter)
        .filter(Filter::named(tr!("browser.files.all").to_string()).pattern("*"))
}

fn answered(request: PickFiles, chosen: rikka_dialog::Result<ChooserResponse>) {
    let files = match chosen {
        Ok(ChooserResponse::Files(files)) if !files.is_empty() => files,
        Ok(_) => return request.dismiss(),
        Err(error) => {
            tracing::warn!(%error, "failed to open file chooser");
            return request.dismiss();
        }
    };
    let files = match request.mode {
        FileMode::Folder => files.into_iter().take(1).collect(),
        FileMode::Single | FileMode::Multiple => files,
    };
    if let Err(error) = request.choose(files) {
        tracing::warn!(%error, "failed to send chosen files");
    }
}

impl Shell {
    pub(crate) fn files_asked(&mut self, tab: TabId, request: PickFiles) {
        let busy = {
            let page_services = self.browser().page_services();
            let mut requests = page_services.requests_mut();
            let tab_requests = requests.edit(tab);
            std::mem::replace(&mut tab_requests.picking_files, true)
        };
        if busy {
            request.dismiss();
            return;
        }
        let chooser = chooser(&request);
        spawn_blocking(
            move || (request, chooser.run()),
            move |shell: &mut Shell,
                  (request, chosen): (PickFiles, rikka_dialog::Result<ChooserResponse>)| {
                if let Some(requests) = shell
                    .browser()
                    .page_services()
                    .requests_mut()
                    .edit_existing(tab)
                {
                    requests.picking_files = false;
                }
                answered(request, chosen);
            },
        );
    }
}
