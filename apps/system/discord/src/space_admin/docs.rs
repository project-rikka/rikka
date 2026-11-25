use discord_core::MAX_WELCOME_CHANNELS;
use rikka_app::prelude::*;

pub(crate) struct Docs {
    pub(super) name: TextDocument,
    pub(super) description: TextDocument,
    pub(super) role_name: TextDocument,
    pub(super) role_emoji: TextDocument,
    pub(super) vanity: TextDocument,
    pub(super) search: TextDocument,
    pub(super) ban_filter: TextDocument,
    pub(super) template_name: TextDocument,
    pub(super) template_description: TextDocument,
    pub(super) upload_name: TextDocument,
    pub(super) upload_tags: TextDocument,
    pub(super) upload_description: TextDocument,
    pub(super) rename: TextDocument,
    pub(super) hook_name: TextDocument,
    pub(super) rule_name: TextDocument,
    pub(super) rule_keywords: TextDocument,
    pub(super) rule_patterns: TextDocument,
    pub(super) rule_allowed: TextDocument,
    pub(super) rule_message: TextDocument,
    pub(super) welcome_description: TextDocument,
    pub(super) welcome_channels: [TextDocument; MAX_WELCOME_CHANNELS],
    pub(super) prompt_title: TextDocument,
    pub(super) option_titles: Vec<TextDocument>,
    pub(super) option_descriptions: Vec<TextDocument>,
}

impl Default for Docs {
    fn default() -> Docs {
        let blank = || TextDocument::new("");
        Docs {
            name: blank(),
            description: blank(),
            role_name: blank(),
            role_emoji: blank(),
            vanity: blank(),
            search: blank(),
            ban_filter: blank(),
            template_name: blank(),
            template_description: blank(),
            upload_name: blank(),
            upload_tags: blank(),
            upload_description: blank(),
            rename: blank(),
            hook_name: blank(),
            rule_name: blank(),
            rule_keywords: blank(),
            rule_patterns: blank(),
            rule_allowed: blank(),
            rule_message: blank(),
            welcome_description: blank(),
            welcome_channels: std::array::from_fn(|_| blank()),
            prompt_title: blank(),
            option_titles: Vec::new(),
            option_descriptions: Vec::new(),
        }
    }
}

pub(super) fn listed(document: &TextDocument) -> Vec<String> {
    document
        .text()
        .split([',', '\n'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect()
}

pub(super) fn joined(entries: &[String]) -> String {
    entries.join(", ")
}
