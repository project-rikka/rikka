use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;

use rikka_proton_account::AddressId;
use rikka_proton_api::{Multipart, ResponseCode, parallel_map_each};
use rikka_proton_pgp::PrivateKey;
use serde::{Deserialize, Serialize};

use crate::client::{MESSAGES, MailClient};
use crate::error::{Error, Result};
use crate::flags::MessageFlags;
use crate::identifier::{LabelId, MessageId};
use crate::import::encryption::encrypt_for_import;

const MAX_MESSAGES_PER_REQUEST: usize = 10;
const MAX_REQUEST_BYTES: usize = 70 * 1024 * 1024;
const NAMES: [&str; MAX_MESSAGES_PER_REQUEST] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
const BLOB: &str = "blob";

#[derive(Clone)]
pub struct ImportMessage {
    literal: Vec<u8>,
    address: AddressId,
    labels: Vec<LabelId>,
    unread: bool,
    flags: MessageFlags,
}

impl ImportMessage {
    #[must_use]
    pub fn new(literal: impl Into<Vec<u8>>, address: &AddressId) -> ImportMessage {
        ImportMessage {
            literal: literal.into(),
            address: address.clone(),
            labels: Vec::new(),
            unread: false,
            flags: MessageFlags::RECEIVED,
        }
    }

    #[must_use]
    pub fn label(mut self, label: &LabelId) -> ImportMessage {
        if !self.labels.contains(label) {
            self.labels.push(label.clone());
        }
        self
    }

    #[must_use]
    pub fn unread(mut self, unread: bool) -> ImportMessage {
        self.unread = unread;
        self
    }

    #[must_use]
    pub fn flags(mut self, flags: MessageFlags) -> ImportMessage {
        self.flags = flags;
        self
    }
}

impl fmt::Debug for ImportMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImportMessage")
            .field("bytes", &self.literal.len())
            .field("address", &self.address)
            .field("labels", &self.labels)
            .field("unread", &self.unread)
            .field("flags", &self.flags)
            .finish()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ImportOutcome {
    #[serde(rename = "Code", default)]
    code: Option<ResponseCode>,
    #[serde(rename = "Error", default)]
    error: Option<String>,
    #[serde(rename = "MessageID", default)]
    message_id: Option<MessageId>,
}

impl ImportOutcome {
    #[must_use]
    pub fn message_id(&self) -> Option<&MessageId> {
        self.message_id.as_ref().filter(|id| !id.is_empty())
    }

    #[must_use]
    pub fn code(&self) -> Option<ResponseCode> {
        self.code
    }

    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref().filter(|error| !error.is_empty())
    }

    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.code.is_some_and(ResponseCode::is_ok) && self.message_id().is_some()
    }
}

#[must_use]
#[derive(Debug, Clone)]
pub struct Import {
    client: MailClient,
    messages: Vec<ImportMessage>,
}

impl Import {
    pub(crate) fn new(client: MailClient) -> Import {
        Import {
            client,
            messages: Vec::new(),
        }
    }

    pub fn message(mut self, message: ImportMessage) -> Self {
        self.messages.push(message);
        self
    }

    pub fn messages(mut self, messages: impl IntoIterator<Item = ImportMessage>) -> Self {
        self.messages.extend(messages);
        self
    }

    pub async fn run(self) -> Result<Vec<ImportOutcome>> {
        let pgp = *self.client.pgp();
        let jobs = self
            .messages
            .iter()
            .map(|message| {
                let (_, key) = self.client.sending_key(&message.address)?;
                Ok((message.literal.clone(), key))
            })
            .collect::<Result<Vec<(Vec<u8>, PrivateKey)>>>()?;

        let encrypted = parallel_map_each(jobs, move |(literal, key)| {
            encrypt_for_import(&pgp, &key, &literal)
        })
        .await
        .into_iter()
        .collect::<Result<Vec<_>>>()?;

        if let Some(index) = encrypted
            .iter()
            .position(|bytes| bytes.len() > MAX_REQUEST_BYTES)
        {
            return Err(Error::ImportTooLarge(index));
        }

        let mut outcomes = Vec::with_capacity(encrypted.len());
        for range in request_ranges(&encrypted) {
            outcomes.extend(
                self.send(&self.messages[range.clone()], &encrypted[range])
                    .await?,
            );
        }

        Ok(outcomes)
    }

    async fn send(
        &self,
        messages: &[ImportMessage],
        encrypted: &[Vec<u8>],
    ) -> Result<Vec<ImportOutcome>> {
        let mut form = Multipart::new();
        let mut metadata = BTreeMap::new();

        for (index, (message, bytes)) in messages.iter().zip(encrypted).enumerate() {
            let name = NAMES[index];
            let mut part = bytes.clone();
            part.extend_from_slice(b"\r\n");
            form = form.file(name, BLOB, part);
            metadata.insert(
                name,
                Metadata {
                    address: &message.address,
                    labels: &message.labels,
                    unread: message.unread,
                    flags: message.flags,
                },
            );
        }

        let metadata_json = serde_json::to_string(&metadata).map_err(|source| Error::Encoding {
            what: "import metadata",
            source,
        })?;
        let form = form.text("Metadata", metadata_json);

        let imported: Vec<NamedImport> = self
            .client
            .api()
            .post(format!("{MESSAGES}/import"))
            .multipart(form)
            .send_field_or_default("Responses")
            .await?;

        let mut by_name: BTreeMap<String, ImportOutcome> = imported
            .into_iter()
            .map(|named| (named.name, named.response))
            .collect();

        Ok(NAMES[..messages.len()]
            .iter()
            .map(|name| by_name.remove(*name).unwrap_or_default())
            .collect())
    }
}

fn request_ranges(encrypted: &[Vec<u8>]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;

    while start < encrypted.len() {
        let mut end = start;
        let mut size = 0;
        while end < encrypted.len()
            && end - start < MAX_MESSAGES_PER_REQUEST
            && size + encrypted[end].len() <= MAX_REQUEST_BYTES
        {
            size += encrypted[end].len();
            end += 1;
        }
        ranges.push(start..end);
        start = end;
    }

    ranges
}

#[derive(Serialize)]
struct Metadata<'a> {
    #[serde(rename = "AddressID")]
    address: &'a AddressId,
    #[serde(rename = "LabelIDs")]
    labels: &'a [LabelId],
    #[serde(
        rename = "Unread",
        serialize_with = "rikka_proton_api::flag::serialize"
    )]
    unread: bool,
    #[serde(rename = "Flags")]
    flags: MessageFlags,
}

#[derive(Deserialize)]
struct NamedImport {
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Response", default)]
    response: ImportOutcome,
}
