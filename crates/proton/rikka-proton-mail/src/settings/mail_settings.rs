use rikka_proton_contacts::{
    MailDefaults, MimeType as ContactMimeType, PgpScheme as ContactScheme,
};
use serde::{Deserialize, Serialize};

use crate::mime::MimeType;
use crate::settings::choices::{
    AutoReplyRepeat, ComposerMode, ImageProxy, MessageButtons, NextMessageOnMove, PgpScheme,
    PmSignature, ShowImages, ShowMoved, SpamAction, SwipeAction, ToolbarAction, ViewLayout,
    ViewMode,
};
use crate::settings::setting::MailSetting;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AutoResponder {
    #[serde(rename = "StartTime")]
    start_time: i64,
    #[serde(rename = "EndTime")]
    end_time: i64,
    #[serde(rename = "Repeat")]
    repeat: AutoReplyRepeat,
    #[serde(rename = "DaysSelected")]
    days: Vec<u8>,
    #[serde(rename = "Subject")]
    subject: String,
    #[serde(rename = "Message")]
    message: String,
    #[serde(
        rename = "IsEnabled",
        deserialize_with = "crate::settings::flag_or_off::deserialize"
    )]
    enabled: bool,
    #[serde(rename = "Zone")]
    zone: String,
}

impl Default for AutoResponder {
    fn default() -> AutoResponder {
        AutoResponder {
            start_time: 0,
            end_time: 0,
            repeat: AutoReplyRepeat::Fixed,
            days: Vec::new(),
            subject: "Auto".to_owned(),
            message: String::new(),
            enabled: false,
            zone: "Europe/Zurich".to_owned(),
        }
    }
}

impl AutoResponder {
    #[must_use]
    pub fn new(subject: impl Into<String>, message: impl Into<String>) -> AutoResponder {
        AutoResponder {
            subject: subject.into(),
            message: message.into(),
            enabled: true,
            ..AutoResponder::default()
        }
    }

    #[must_use]
    pub fn window(mut self, start_time: i64, end_time: i64) -> AutoResponder {
        self.start_time = start_time;
        self.end_time = end_time;
        self
    }

    #[must_use]
    pub fn repeating(
        mut self,
        repeat: AutoReplyRepeat,
        days: impl IntoIterator<Item = u8>,
    ) -> AutoResponder {
        self.repeat = repeat;
        self.days = days.into_iter().collect();
        self
    }

    #[must_use]
    pub fn zone(mut self, zone: impl Into<String>) -> AutoResponder {
        self.zone = zone.into();
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> AutoResponder {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub fn start_time(&self) -> i64 {
        self.start_time
    }

    #[must_use]
    pub fn end_time(&self) -> i64 {
        self.end_time
    }

    #[must_use]
    pub fn repeat(&self) -> AutoReplyRepeat {
        self.repeat
    }

    #[must_use]
    pub fn days(&self) -> &[u8] {
        &self.days
    }

    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn time_zone(&self) -> &str {
        &self.zone
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Toolbar {
    #[serde(rename = "IsCustom")]
    custom: bool,
    #[serde(rename = "Actions")]
    actions: Vec<ToolbarAction>,
}

impl Toolbar {
    #[must_use]
    pub fn is_custom(&self) -> bool {
        self.custom
    }

    #[must_use]
    pub fn actions(&self) -> &[ToolbarAction] {
        &self.actions
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MobileSettings {
    #[serde(rename = "ListToolbar")]
    list: Toolbar,
    #[serde(rename = "MessageToolbar")]
    message: Toolbar,
    #[serde(rename = "ConversationToolbar")]
    conversation: Toolbar,
}

impl MobileSettings {
    #[must_use]
    pub fn new(
        list: impl IntoIterator<Item = ToolbarAction>,
        message: impl IntoIterator<Item = ToolbarAction>,
        conversation: impl IntoIterator<Item = ToolbarAction>,
    ) -> MobileSettings {
        let custom = |actions: Vec<ToolbarAction>| Toolbar {
            custom: true,
            actions,
        };
        MobileSettings {
            list: custom(list.into_iter().collect()),
            message: custom(message.into_iter().collect()),
            conversation: custom(conversation.into_iter().collect()),
        }
    }

    #[must_use]
    pub fn list(&self) -> &Toolbar {
        &self.list
    }

    #[must_use]
    pub fn message(&self) -> &Toolbar {
        &self.message
    }

    #[must_use]
    pub fn conversation(&self) -> &Toolbar {
        &self.conversation
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MailSettings {
    #[serde(rename = "DisplayName")]
    display_name: String,
    #[serde(rename = "Signature")]
    signature: String,
    #[serde(rename = "AutoResponder")]
    auto_responder: AutoResponder,
    #[serde(rename = "AutoSaveContacts", with = "crate::settings::flag_or_off")]
    auto_save_contacts: bool,
    #[serde(rename = "ComposerMode")]
    composer_mode: ComposerMode,
    #[serde(rename = "MessageButtons")]
    message_buttons: MessageButtons,
    #[serde(rename = "ShowImages")]
    show_images: ShowImages,
    #[serde(rename = "HideRemoteImages", with = "crate::settings::flag_or_off")]
    hide_remote_images: bool,
    #[serde(rename = "HideEmbeddedImages", with = "crate::settings::flag_or_off")]
    hide_embedded_images: bool,
    #[serde(rename = "ShowMoved")]
    show_moved: ShowMoved,
    #[serde(rename = "ViewMode")]
    view_mode: ViewMode,
    #[serde(rename = "ViewLayout")]
    view_layout: ViewLayout,
    #[serde(rename = "SwipeLeft")]
    swipe_left: SwipeAction,
    #[serde(rename = "SwipeRight")]
    swipe_right: SwipeAction,
    #[serde(rename = "PageSize")]
    page_size: u32,
    #[serde(rename = "Shortcuts", with = "crate::settings::flag_or_off")]
    shortcuts: bool,
    #[serde(rename = "PMSignature")]
    pm_signature: PmSignature,
    #[serde(
        rename = "PMSignatureReferralLink",
        with = "crate::settings::flag_or_off"
    )]
    pm_signature_referral: bool,
    #[serde(rename = "ImageProxy")]
    image_proxy: ImageProxy,
    #[serde(rename = "RightToLeft", with = "crate::settings::flag_or_off")]
    right_to_left: bool,
    #[serde(rename = "AttachPublicKey", with = "crate::settings::flag_or_off")]
    attach_public_key: bool,
    #[serde(rename = "Sign", with = "crate::settings::flag_or_off")]
    sign: bool,
    #[serde(rename = "PGPScheme")]
    pgp_scheme: PgpScheme,
    #[serde(rename = "PromptPin", with = "crate::settings::flag_or_off")]
    prompt_pin: bool,
    #[serde(rename = "DraftMIMEType")]
    draft_mime_type: MimeType,
    #[serde(rename = "ReceiveMIMEType")]
    receive_mime_type: MimeType,
    #[serde(rename = "ShowMIMEType")]
    show_mime_type: MimeType,
    #[serde(rename = "StickyLabels", with = "crate::settings::flag_or_off")]
    sticky_labels: bool,
    #[serde(rename = "ConfirmLink", with = "crate::settings::flag_or_off")]
    confirm_link: bool,
    #[serde(rename = "DelaySendSeconds")]
    delay_send_seconds: u32,
    #[serde(rename = "EnableFolderColor", with = "crate::settings::flag_or_off")]
    enable_folder_color: bool,
    #[serde(
        rename = "InheritParentFolderColor",
        with = "crate::settings::flag_or_off"
    )]
    inherit_parent_folder_color: bool,
    #[serde(rename = "FontFace")]
    font_face: Option<String>,
    #[serde(rename = "FontSize")]
    font_size: Option<u32>,
    #[serde(rename = "SpamAction")]
    spam_action: Option<SpamAction>,
    #[serde(rename = "BlockSenderConfirmation")]
    block_sender_confirmation: Option<i32>,
    #[serde(rename = "HideSenderImages", with = "crate::settings::flag_or_off")]
    hide_sender_images: bool,
    #[serde(rename = "AutoDeleteSpamAndTrashDays")]
    auto_delete_days: Option<u32>,
    #[serde(rename = "UnreadFavicon", with = "crate::settings::flag_or_off")]
    unread_favicon: bool,
    #[serde(rename = "RecipientLimit")]
    recipient_limit: u32,
    #[serde(rename = "AlmostAllMail", with = "crate::settings::flag_or_off")]
    almost_all_mail: bool,
    #[serde(rename = "NextMessageOnMove")]
    next_message_on_move: NextMessageOnMove,
    #[serde(rename = "RemoveImageMetadata", with = "crate::settings::flag_or_off")]
    remove_image_metadata: bool,
    #[serde(rename = "KT", with = "crate::settings::flag_or_off")]
    key_transparency: bool,
    #[serde(rename = "MailCategoryView", with = "crate::settings::flag_or_off")]
    category_view: bool,
    #[serde(
        rename = "MailCategoryViewCountersEnabled",
        with = "crate::settings::flag_or_off"
    )]
    category_counters: bool,
    #[serde(rename = "MobileSettings")]
    mobile: Option<MobileSettings>,
}

impl Default for MailSettings {
    fn default() -> MailSettings {
        MailSettings {
            display_name: String::new(),
            signature: String::new(),
            auto_responder: AutoResponder::default(),
            auto_save_contacts: true,
            composer_mode: ComposerMode::Popup,
            message_buttons: MessageButtons::ReadFirst,
            show_images: ShowImages::None,
            hide_remote_images: false,
            hide_embedded_images: false,
            show_moved: ShowMoved::None,
            view_mode: ViewMode::Conversations,
            view_layout: ViewLayout::Row,
            swipe_left: SwipeAction::Archive,
            swipe_right: SwipeAction::Trash,
            page_size: 50,
            shortcuts: true,
            pm_signature: PmSignature::Disabled,
            pm_signature_referral: false,
            image_proxy: ImageProxy::Proxy,
            right_to_left: false,
            attach_public_key: false,
            sign: false,
            pgp_scheme: PgpScheme::Mime,
            prompt_pin: false,
            draft_mime_type: MimeType::Html,
            receive_mime_type: MimeType::Html,
            show_mime_type: MimeType::Html,
            sticky_labels: false,
            confirm_link: true,
            delay_send_seconds: 10,
            enable_folder_color: false,
            inherit_parent_folder_color: true,
            font_face: None,
            font_size: None,
            spam_action: None,
            block_sender_confirmation: None,
            hide_sender_images: false,
            auto_delete_days: None,
            unread_favicon: false,
            recipient_limit: 100,
            almost_all_mail: false,
            next_message_on_move: NextMessageOnMove::Default,
            remove_image_metadata: false,
            key_transparency: false,
            category_view: false,
            category_counters: false,
            mobile: None,
        }
    }
}

impl MailSettings {
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    #[must_use]
    pub fn signature(&self) -> &str {
        &self.signature
    }

    #[must_use]
    pub fn auto_responder(&self) -> &AutoResponder {
        &self.auto_responder
    }

    #[must_use]
    pub fn auto_save_contacts(&self) -> bool {
        self.auto_save_contacts
    }

    #[must_use]
    pub fn composer_mode(&self) -> ComposerMode {
        self.composer_mode
    }

    #[must_use]
    pub fn message_buttons(&self) -> MessageButtons {
        self.message_buttons
    }

    #[must_use]
    pub fn show_images(&self) -> ShowImages {
        self.show_images
    }

    #[must_use]
    pub fn hides_remote_images(&self) -> bool {
        self.hide_remote_images
    }

    #[must_use]
    pub fn hides_embedded_images(&self) -> bool {
        self.hide_embedded_images
    }

    #[must_use]
    pub fn show_moved(&self) -> ShowMoved {
        self.show_moved
    }

    #[must_use]
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    #[must_use]
    pub fn view_layout(&self) -> ViewLayout {
        self.view_layout
    }

    #[must_use]
    pub fn swipe_left(&self) -> SwipeAction {
        self.swipe_left
    }

    #[must_use]
    pub fn swipe_right(&self) -> SwipeAction {
        self.swipe_right
    }

    #[must_use]
    pub fn page_size(&self) -> u32 {
        self.page_size
    }

    #[must_use]
    pub fn shortcuts(&self) -> bool {
        self.shortcuts
    }

    #[must_use]
    pub fn pm_signature(&self) -> PmSignature {
        self.pm_signature
    }

    #[must_use]
    pub fn pm_signature_referral(&self) -> bool {
        self.pm_signature_referral
    }

    #[must_use]
    pub fn image_proxy(&self) -> ImageProxy {
        self.image_proxy
    }

    #[must_use]
    pub fn right_to_left(&self) -> bool {
        self.right_to_left
    }

    #[must_use]
    pub fn attaches_public_key(&self) -> bool {
        self.attach_public_key
    }

    #[must_use]
    pub fn signs(&self) -> bool {
        self.sign
    }

    #[must_use]
    pub fn pgp_scheme(&self) -> PgpScheme {
        self.pgp_scheme
    }

    #[must_use]
    pub fn prompt_pin(&self) -> bool {
        self.prompt_pin
    }

    #[must_use]
    pub fn draft_mime_type(&self) -> &MimeType {
        &self.draft_mime_type
    }

    #[must_use]
    pub fn receive_mime_type(&self) -> &MimeType {
        &self.receive_mime_type
    }

    #[must_use]
    pub fn show_mime_type(&self) -> &MimeType {
        &self.show_mime_type
    }

    #[must_use]
    pub fn sticky_labels(&self) -> bool {
        self.sticky_labels
    }

    #[must_use]
    pub fn confirms_links(&self) -> bool {
        self.confirm_link
    }

    #[must_use]
    pub fn delay_send_seconds(&self) -> u32 {
        self.delay_send_seconds
    }

    #[must_use]
    pub fn folder_colors(&self) -> bool {
        self.enable_folder_color
    }

    #[must_use]
    pub fn inherits_parent_folder_color(&self) -> bool {
        self.inherit_parent_folder_color
    }

    #[must_use]
    pub fn font_face(&self) -> Option<&str> {
        self.font_face.as_deref()
    }

    #[must_use]
    pub fn font_size(&self) -> Option<u32> {
        self.font_size
    }

    #[must_use]
    pub fn spam_action(&self) -> Option<SpamAction> {
        self.spam_action
    }

    #[must_use]
    pub fn skips_block_sender_confirmation(&self) -> bool {
        self.block_sender_confirmation.is_some()
    }

    #[must_use]
    pub fn hides_sender_images(&self) -> bool {
        self.hide_sender_images
    }

    #[must_use]
    pub fn auto_delete_days(&self) -> Option<u32> {
        self.auto_delete_days.filter(|days| *days > 0)
    }

    #[must_use]
    pub fn unread_favicon(&self) -> bool {
        self.unread_favicon
    }

    #[must_use]
    pub fn recipient_limit(&self) -> u32 {
        self.recipient_limit
    }

    #[must_use]
    pub fn almost_all_mail(&self) -> bool {
        self.almost_all_mail
    }

    #[must_use]
    pub fn next_message_on_move(&self) -> NextMessageOnMove {
        self.next_message_on_move
    }

    #[must_use]
    pub fn removes_image_metadata(&self) -> bool {
        self.remove_image_metadata
    }

    #[must_use]
    pub fn key_transparency(&self) -> bool {
        self.key_transparency
    }

    #[must_use]
    pub fn category_view(&self) -> bool {
        self.category_view
    }

    #[must_use]
    pub fn category_view_counters(&self) -> bool {
        self.category_counters
    }

    #[must_use]
    pub fn mobile(&self) -> Option<&MobileSettings> {
        self.mobile.as_ref()
    }

    #[must_use]
    pub fn mail_defaults(&self) -> MailDefaults {
        MailDefaults {
            sign: self.sign,
            scheme: match self.pgp_scheme {
                PgpScheme::Inline => ContactScheme::Inline,
                _ => ContactScheme::Mime,
            },
            mime_type: match self.draft_mime_type {
                MimeType::PlainText => ContactMimeType::PlainText,
                _ => ContactMimeType::Html,
            },
        }
    }
    pub fn apply(&mut self, setting: &MailSetting) {
        match setting {
            MailSetting::DisplayName(name) => self.display_name = name.clone(),
            MailSetting::Signature(signature) => self.signature = signature.clone(),
            MailSetting::AutoSaveContacts(on) => self.auto_save_contacts = *on,
            MailSetting::ComposerMode(mode) => self.composer_mode = *mode,
            MailSetting::MessageButtons(order) => self.message_buttons = *order,
            MailSetting::ShowImages(shown) => self.show_images = *shown,
            MailSetting::HideRemoteImages(on) => self.hide_remote_images = *on,
            MailSetting::HideEmbeddedImages(on) => self.hide_embedded_images = *on,
            MailSetting::ImageProxy { proxy, enabled } => {
                let current = i32::from(self.image_proxy);
                let bits = i32::from(*proxy);
                self.image_proxy = ImageProxy::from(match *enabled {
                    true => current | bits,
                    false => current & !bits,
                });
            }
            MailSetting::ShowMoved(moved) => self.show_moved = *moved,
            MailSetting::ViewMode(mode) => self.view_mode = *mode,
            MailSetting::ViewLayout(layout) => self.view_layout = *layout,
            MailSetting::SwipeLeft(action) => self.swipe_left = *action,
            MailSetting::SwipeRight(action) => self.swipe_right = *action,
            MailSetting::PageSize(size) => self.page_size = *size,
            MailSetting::PmSignature(state) => self.pm_signature = *state,
            MailSetting::PmSignatureReferral(on) => self.pm_signature_referral = *on,
            MailSetting::DraftMimeType(kind) => self.draft_mime_type = kind.clone(),
            MailSetting::ReceiveMimeType(kind) => self.receive_mime_type = kind.clone(),
            MailSetting::ShowMimeType(kind) => self.show_mime_type = kind.clone(),
            MailSetting::RightToLeft(on) => self.right_to_left = *on,
            MailSetting::AttachPublicKey(on) => self.attach_public_key = *on,
            MailSetting::Sign(on) => self.sign = *on,
            MailSetting::PgpScheme(scheme) => self.pgp_scheme = *scheme,
            MailSetting::PromptPin(on) => self.prompt_pin = *on,
            MailSetting::StickyLabels(on) => self.sticky_labels = *on,
            MailSetting::ConfirmLink(on) => self.confirm_link = *on,
            MailSetting::Shortcuts(on) => self.shortcuts = *on,
            MailSetting::DelaySendSeconds(seconds) => self.delay_send_seconds = *seconds,
            MailSetting::EnableFolderColor(on) => self.enable_folder_color = *on,
            MailSetting::InheritParentFolderColor(on) => self.inherit_parent_folder_color = *on,
            MailSetting::FontFace(face) => self.font_face = face.clone(),
            MailSetting::FontSize(size) => self.font_size = *size,
            MailSetting::SpamAction(action) => self.spam_action = *action,
            MailSetting::SkipBlockSenderConfirmation(skip) => {
                self.block_sender_confirmation = skip.then_some(1);
            }
            MailSetting::HideSenderImages(on) => self.hide_sender_images = *on,
            MailSetting::AutoDeleteDays(days) => self.auto_delete_days = *days,
            MailSetting::UnreadFavicon(on) => self.unread_favicon = *on,
            MailSetting::AlmostAllMail(on) => self.almost_all_mail = *on,
            MailSetting::NextMessageOnMove(choice) => self.next_message_on_move = *choice,
            MailSetting::RemoveImageMetadata(on) => self.remove_image_metadata = *on,
            MailSetting::KeyTransparency(on) => self.key_transparency = *on,
            MailSetting::CategoryView(on) => self.category_view = *on,
            MailSetting::CategoryViewCounters(on) => self.category_counters = *on,
            MailSetting::AutoResponder(responder) => self.auto_responder = responder.clone(),
            MailSetting::Mobile(mobile) => self.mobile = Some(mobile.clone()),
        }
    }
}
