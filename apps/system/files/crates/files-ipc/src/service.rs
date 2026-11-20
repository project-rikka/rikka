use zbus::blocking::Connection;
use zbus::fdo::RequestNameFlags;
use zbus::fdo::RequestNameReply;

use crate::request::{Request, Reveal, Sink};
use crate::{DESKTOP_NAME, DESKTOP_PATH};

#[derive(Debug)]
pub struct Service {
    _connection: Connection,
    desktop_name: bool,
}

impl Service {
    #[must_use]
    pub fn is_desktop_file_manager(&self) -> bool {
        self.desktop_name
    }
}

pub fn start(sink: Sink) -> zbus::Result<Service> {
    let connection = Connection::session()?;
    connection
        .object_server()
        .at(DESKTOP_PATH, FileManager1 { sink })?;
    let desktop_name = connection
        .request_name_with_flags(DESKTOP_NAME, RequestNameFlags::DoNotQueue.into())
        .map(|reply| reply == RequestNameReply::PrimaryOwner)
        .unwrap_or(false);

    Ok(Service {
        _connection: connection,
        desktop_name,
    })
}

struct FileManager1 {
    sink: Sink,
}

impl FileManager1 {
    fn reveal(&self, reveal: Option<Reveal>, startup_id: String) {
        let Some(reveal) = reveal else {
            return;
        };
        (self.sink)(Request {
            reveal,
            activation_token: (!startup_id.is_empty()).then_some(startup_id),
        });
    }
}

#[zbus::interface(name = "org.freedesktop.FileManager1")]
impl FileManager1 {
    fn show_folders(&self, uris: Vec<String>, startup_id: String) {
        self.reveal(Reveal::from_uris(&uris, Reveal::Folders), startup_id);
    }

    fn show_items(&self, uris: Vec<String>, startup_id: String) {
        self.reveal(Reveal::from_uris(&uris, Reveal::Items), startup_id);
    }

    fn show_item_properties(&self, uris: Vec<String>, startup_id: String) {
        self.reveal(Reveal::from_uris(&uris, Reveal::Properties), startup_id);
    }
}
