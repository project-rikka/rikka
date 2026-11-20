use files_dialog::{Ask, Kind};
use rikka_portal_backend::{Helper, HelperOutcome};
use zbus::interface;
use zbus::object_server::ObjectServer;
use zbus::zvariant::OwnedObjectPath;

use crate::options::Options;
use crate::results::Results;

#[derive(Debug)]
pub(crate) struct FileChooser {
    dialog: Helper,
}

impl FileChooser {
    pub(crate) fn new(dialog: Helper) -> Self {
        Self { dialog }
    }

    async fn show(
        &self,
        server: &ObjectServer,
        handle: OwnedObjectPath,
        ask: Ask,
    ) -> (u32, Results) {
        let shown = self.dialog.ask(server, &handle, &ask).await;
        let results = match &shown {
            HelperOutcome::Answered(answer) => Results::new(&ask, answer),
            HelperOutcome::Cancelled | HelperOutcome::Ended => Results::default(),
        };
        (shown.code(), results)
    }
}

#[interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    async fn open_file(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, Results) {
        drop(app_id);
        let ask = options.into_ask(Kind::Open, title, &parent_window);
        self.show(server, handle, ask).await
    }

    async fn save_file(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, Results) {
        drop(app_id);
        let ask = options.into_ask(Kind::Save, title, &parent_window);
        self.show(server, handle, ask).await
    }

    async fn save_files(
        &self,
        handle: OwnedObjectPath,
        app_id: String,
        parent_window: String,
        title: String,
        options: Options,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> (u32, Results) {
        drop(app_id);
        let ask = options.into_ask(Kind::SaveInto, title, &parent_window);
        self.show(server, handle, ask).await
    }
}
