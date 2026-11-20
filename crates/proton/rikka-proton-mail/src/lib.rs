mod address;
mod attachment;
mod batch;
mod cached;
mod client;
mod conversation;
mod count;
mod decrypted;
mod direct;
mod draft;
mod error;
mod event;
mod feed;
mod filter;
mod flags;
mod identifier;
mod import;
mod label;
mod label_draft;
mod label_query;
mod label_selection;
mod listing;
mod message;
mod message_ids;
mod mime;
mod opened_message;
mod outcome;
mod outside;
mod package;
mod packaging;
mod query;
mod recipients;
mod search;
mod send;
mod send_options;
mod settings;
mod sort;
mod system_label;
mod upload;

pub use address::MailAddress;
pub use attachment::{Attachment, Disposition};
pub use batch::{Batch, BatchTarget, ConversationsInLabel};
pub use client::MailClient;
pub use conversation::{Conversation, ConversationLabel, ConversationThread};
pub use count::LabelCount;
pub use decrypted::{DecryptedAttachment, DecryptedMessage};
pub use direct::{DirectAction, DirectSend, DirectSent};
pub use draft::{DraftAction, DraftBuilder};
pub use error::{Error, Result};
pub use event::{AddressEvent, ConversationEvent, Event, LabelEvent, MessageEvent};
pub use filter::{
    Comparator, Condition, ConditionKind, Filter, FilterActions, FilterDraft, Operator,
    SieveArgument, SieveCommand, SieveIssue, SievePosition, SieveScript, SieveSyntaxError,
    SieveTest, SimpleFilter,
};
pub use flags::MessageFlags;
pub use identifier::{AttachmentId, ConversationId, FilterId, LabelId, MessageId};
pub use import::{Import, ImportMessage, ImportOutcome};
pub use label::{Label, LabelType};
pub use label_draft::LabelDraft;
pub use label_query::LabelQuery;
pub use label_selection::LabelSelection;
pub use message::{Message, MessageMetadata};
pub use message_ids::MessageIds;
pub use mime::{
    ContentDisposition, ContentType, HeaderField, Headers, MULTIPART_MIXED, MimeContent, MimeFile,
    MimeMessage, MimePart, MimeType, Multipart, Parameters, PartBody, TEXT_HTML, TEXT_PLAIN,
    html_to_text,
};
pub use opened_message::OpenedMessage;
pub use outcome::{BatchOutcome, UndoToken};
pub use outside::{OutsideClient, OutsideMessage, OutsideReply, OutsideSession, SentReply};
pub use package::{
    AddressPackage, ExposedKey, OutsideAuth, Package, PackageKind, PackageTypes, SignatureMode,
};
pub use query::{ConversationQuery, Conversations, MessageQuery, Messages, Query, QueryKind};
pub use rikka_proton_account::EventId;
pub use rikka_proton_api::EventAction;
pub use search::{IndexStep, Indexer, SearchIndex, SearchQuery};
pub use send::SendBuilder;
pub use settings::{
    AutoReplyRepeat, AutoResponder, ComposerMode, ImageProxy, MailSetting, MailSettings,
    MessageButtons, MobileSettings, NextMessageOnMove, PgpScheme, PmSignature, ShowImages,
    ShowMoved, SpamAction, SwipeAction, Toolbar, ToolbarAction, ViewLayout, ViewMode,
};
pub use sort::Sort;
pub use system_label::SystemLabel;
pub use upload::AttachmentUpload;
