use serde_json::{Value, json};

use crate::mime::MimeType;
use crate::settings::choices::{
    ComposerMode, ImageProxy, MessageButtons, NextMessageOnMove, PgpScheme, PmSignature,
    ShowImages, ShowMoved, SpamAction, SwipeAction, ViewLayout, ViewMode,
};
use crate::settings::mail_settings::{AutoResponder, MobileSettings};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MailSetting {
    DisplayName(String),
    Signature(String),
    AutoSaveContacts(bool),
    ComposerMode(ComposerMode),
    MessageButtons(MessageButtons),
    ShowImages(ShowImages),
    HideRemoteImages(bool),
    HideEmbeddedImages(bool),
    ImageProxy { proxy: ImageProxy, enabled: bool },
    ShowMoved(ShowMoved),
    ViewMode(ViewMode),
    ViewLayout(ViewLayout),
    SwipeLeft(SwipeAction),
    SwipeRight(SwipeAction),
    PageSize(u32),
    PmSignature(PmSignature),
    PmSignatureReferral(bool),
    DraftMimeType(MimeType),
    ReceiveMimeType(MimeType),
    ShowMimeType(MimeType),
    RightToLeft(bool),
    AttachPublicKey(bool),
    Sign(bool),
    PgpScheme(PgpScheme),
    PromptPin(bool),
    StickyLabels(bool),
    ConfirmLink(bool),
    Shortcuts(bool),
    DelaySendSeconds(u32),
    EnableFolderColor(bool),
    InheritParentFolderColor(bool),
    FontFace(Option<String>),
    FontSize(Option<u32>),
    SpamAction(Option<SpamAction>),
    SkipBlockSenderConfirmation(bool),
    HideSenderImages(bool),
    AutoDeleteDays(Option<u32>),
    UnreadFavicon(bool),
    AlmostAllMail(bool),
    NextMessageOnMove(NextMessageOnMove),
    RemoveImageMetadata(bool),
    KeyTransparency(bool),
    CategoryView(bool),
    CategoryViewCounters(bool),
    AutoResponder(AutoResponder),
    Mobile(MobileSettings),
}

impl MailSetting {
    pub(crate) fn path(&self) -> &'static str {
        match self {
            MailSetting::DisplayName(_) => "display",
            MailSetting::Signature(_) => "signature",
            MailSetting::AutoSaveContacts(_) => "autocontacts",
            MailSetting::ComposerMode(_) => "composermode",
            MailSetting::MessageButtons(_) => "messagebuttons",
            MailSetting::ShowImages(_) => "images",
            MailSetting::HideRemoteImages(_) => "hide-remote-images",
            MailSetting::HideEmbeddedImages(_) => "hide-embedded-images",
            MailSetting::ImageProxy { .. } => "imageproxy",
            MailSetting::ShowMoved(_) => "moved",
            MailSetting::ViewMode(_) => "viewmode",
            MailSetting::ViewLayout(_) => "viewlayout",
            MailSetting::SwipeLeft(_) => "swipeleft",
            MailSetting::SwipeRight(_) => "swiperight",
            MailSetting::PageSize(_) => "pagesize",
            MailSetting::PmSignature(_) => "pmsignature",
            MailSetting::PmSignatureReferral(_) => "pmsignature-referral",
            MailSetting::DraftMimeType(_) => "drafttype",
            MailSetting::ReceiveMimeType(_) => "receivetype",
            MailSetting::ShowMimeType(_) => "showtype",
            MailSetting::RightToLeft(_) => "righttoleft",
            MailSetting::AttachPublicKey(_) => "attachpublic",
            MailSetting::Sign(_) => "sign",
            MailSetting::PgpScheme(_) => "pgpscheme",
            MailSetting::PromptPin(_) => "promptpin",
            MailSetting::StickyLabels(_) => "stickylabels",
            MailSetting::ConfirmLink(_) => "confirmlink",
            MailSetting::Shortcuts(_) => "shortcuts",
            MailSetting::DelaySendSeconds(_) => "delaysend",
            MailSetting::EnableFolderColor(_) => "enablefoldercolor",
            MailSetting::InheritParentFolderColor(_) => "inheritparentfoldercolor",
            MailSetting::FontFace(_) => "fontface",
            MailSetting::FontSize(_) => "fontsize",
            MailSetting::SpamAction(_) => "spam-action",
            MailSetting::SkipBlockSenderConfirmation(_) => "block-sender-confirmation",
            MailSetting::HideSenderImages(_) => "hide-sender-images",
            MailSetting::AutoDeleteDays(_) => "auto-delete-spam-and-trash-days",
            MailSetting::UnreadFavicon(_) => "unread-favicon",
            MailSetting::AlmostAllMail(_) => "almost-all-mail",
            MailSetting::NextMessageOnMove(_) => "next-message-on-move",
            MailSetting::RemoveImageMetadata(_) => "remove-image-metadata",
            MailSetting::KeyTransparency(_) => "kt",
            MailSetting::CategoryView(_) => "mail-category-view",
            MailSetting::CategoryViewCounters(_) => "mail-category-view-counters-enabled",
            MailSetting::AutoResponder(_) => "autoresponder",
            MailSetting::Mobile(_) => "mobilesettings",
        }
    }

    pub(crate) fn body(&self) -> Value {
        let flag = u8::from;
        match self {
            MailSetting::DisplayName(name) => json!({ "DisplayName": name }),
            MailSetting::Signature(signature) => json!({ "Signature": signature }),
            MailSetting::AutoSaveContacts(on) => json!({ "AutoSaveContacts": flag(*on) }),
            MailSetting::ComposerMode(mode) => json!({ "ComposerMode": mode }),
            MailSetting::MessageButtons(order) => json!({ "MessageButtons": order }),
            MailSetting::ShowImages(shown) => json!({ "ShowImages": shown }),
            MailSetting::HideRemoteImages(on) => json!({ "HideRemoteImages": flag(*on) }),
            MailSetting::HideEmbeddedImages(on) => json!({ "HideEmbeddedImages": flag(*on) }),
            MailSetting::ImageProxy { proxy, enabled } => {
                json!({ "ImageProxy": proxy, "Action": flag(*enabled) })
            }
            MailSetting::ShowMoved(moved) => json!({ "ShowMoved": moved }),
            MailSetting::ViewMode(mode) => json!({ "ViewMode": mode }),
            MailSetting::ViewLayout(layout) => json!({ "ViewLayout": layout }),
            MailSetting::SwipeLeft(action) => json!({ "SwipeLeft": action }),
            MailSetting::SwipeRight(action) => json!({ "SwipeRight": action }),
            MailSetting::PageSize(size) => json!({ "PageSize": size }),
            MailSetting::PmSignature(state) => json!({ "PMSignature": state }),
            MailSetting::PmSignatureReferral(on) => {
                json!({ "PMSignatureReferralLink": flag(*on) })
            }
            MailSetting::DraftMimeType(kind)
            | MailSetting::ReceiveMimeType(kind)
            | MailSetting::ShowMimeType(kind) => json!({ "MIMEType": kind }),
            MailSetting::RightToLeft(on) => json!({ "RightToLeft": flag(*on) }),
            MailSetting::AttachPublicKey(on) => json!({ "AttachPublicKey": flag(*on) }),
            MailSetting::Sign(on) => json!({ "Sign": flag(*on) }),
            MailSetting::PgpScheme(scheme) => json!({ "PGPScheme": scheme }),
            MailSetting::PromptPin(on) => json!({ "PromptPin": flag(*on) }),
            MailSetting::StickyLabels(on) => json!({ "StickyLabels": flag(*on) }),
            MailSetting::ConfirmLink(on) => json!({ "ConfirmLink": flag(*on) }),
            MailSetting::Shortcuts(on) => json!({ "Shortcuts": flag(*on) }),
            MailSetting::DelaySendSeconds(seconds) => json!({ "DelaySendSeconds": seconds }),
            MailSetting::EnableFolderColor(on) => json!({ "EnableFolderColor": flag(*on) }),
            MailSetting::InheritParentFolderColor(on) => {
                json!({ "InheritParentFolderColor": flag(*on) })
            }
            MailSetting::FontFace(face) => json!({ "FontFace": face }),
            MailSetting::FontSize(size) => json!({ "FontSize": size }),
            MailSetting::SpamAction(action) => json!({ "SpamAction": action }),
            MailSetting::SkipBlockSenderConfirmation(skip) => {
                json!({ "BlockSenderConfirmation": skip.then_some(1) })
            }
            MailSetting::HideSenderImages(on) => json!({ "HideSenderImages": flag(*on) }),
            MailSetting::AutoDeleteDays(days) => json!({ "Days": days.unwrap_or(0) }),
            MailSetting::UnreadFavicon(on) => json!({ "UnreadFavicon": flag(*on) }),
            MailSetting::AlmostAllMail(on) => json!({ "AlmostAllMail": flag(*on) }),
            MailSetting::NextMessageOnMove(choice) => json!({ "NextMessageOnMove": choice }),
            MailSetting::RemoveImageMetadata(on) => json!({ "RemoveImageMetadata": on }),
            MailSetting::KeyTransparency(on) => json!({ "KT": flag(*on) }),
            MailSetting::CategoryView(on) => json!({ "MailCategoryView": on }),
            MailSetting::CategoryViewCounters(on) => {
                json!({ "MailCategoryViewCountersEnabled": on })
            }
            MailSetting::AutoResponder(responder) => json!({ "AutoResponder": responder }),
            MailSetting::Mobile(mobile) => json!({
                "ListToolbar": mobile.list().actions(),
                "MessageToolbar": mobile.message().actions(),
                "ConversationToolbar": mobile.conversation().actions(),
            }),
        }
    }
}
