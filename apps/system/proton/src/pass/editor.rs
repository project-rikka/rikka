use proton_session::{ErrorBanner, Failure};
use proton_vaults::{
    Card, CustomField, Draft, Group, IdentityField, IdentitySection, ItemType, MAX_PASSWORD_LENGTH,
    MIN_PASSWORD_LENGTH, PasswordOptions, Ssh, Vault, Wifi, alias_prefix, generate_password,
};
use rikka_app::prelude::*;
use rikka_proton_pass::{AliasMailbox, AliasOptions, ShareId};
use rikka_widgets::{column, row};

use crate::pane;
use crate::pass::{Pass, detail, list};

#[derive(Debug)]
pub(crate) struct Made {
    pub(crate) prefix: String,
    pub(crate) suffix: String,
    pub(crate) mailboxes: Vec<i64>,
}

#[derive(Debug)]
struct Writing {
    name: TextDocument,
    value: TextDocument,
    hidden: bool,
}

#[derive(Debug)]
struct Sheet {
    name: TextDocument,
    fields: Vec<Writing>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Where {
    Loose,
    In(usize),
}

#[derive(Debug, State)]
pub(crate) struct Editor {
    item_type: ItemType,
    id: Option<rikka_proton_pass::ItemId>,
    title: TextDocument,
    note: TextDocument,
    email: TextDocument,
    username: TextDocument,
    password: TextDocument,
    totp: TextDocument,
    urls: TextDocument,
    holder: TextDocument,
    number: TextDocument,
    verification: TextDocument,
    expiration: TextDocument,
    pin: TextDocument,
    ssid: TextDocument,
    wifi_password: TextDocument,
    security: usize,
    public: TextDocument,
    private: TextDocument,
    prefix: TextDocument,
    suffix: usize,
    choices: Option<AliasOptions>,
    mailboxes: Vec<i64>,
    traits: Vec<TextDocument>,
    fields: Vec<Writing>,
    groups: Vec<Sheet>,
    recipe: PasswordOptions,
    revealed: bool,
    busy: bool,
    error: ErrorBanner,
    origin: Draft,
}

impl Editor {
    pub(crate) fn from_draft(draft: Draft) -> Editor {
        Editor {
            item_type: draft.item_type,
            id: draft.id.clone(),
            title: TextDocument::new(&draft.title),
            note: TextDocument::new(&draft.note),
            email: TextDocument::new(&draft.email),
            username: TextDocument::new(&draft.username),
            password: TextDocument::new(&draft.password),
            totp: TextDocument::new(&draft.totp),
            urls: TextDocument::new(&draft.urls),
            holder: TextDocument::new(&draft.card.holder),
            number: TextDocument::new(&draft.card.number),
            verification: TextDocument::new(&draft.card.verification),
            expiration: TextDocument::new(&draft.card.expiration),
            pin: TextDocument::new(&draft.card.pin),
            ssid: TextDocument::new(&draft.wifi.ssid),
            wifi_password: TextDocument::new(&draft.wifi.password),
            security: usize::try_from(draft.wifi.security.max(0)).unwrap_or_default(),
            public: TextDocument::new(&draft.ssh.public),
            private: TextDocument::new(&draft.ssh.private),
            prefix: TextDocument::new(""),
            suffix: 0,
            choices: None,
            mailboxes: Vec::new(),
            traits: match draft.item_type {
                ItemType::Identity => IdentityField::ALL
                    .iter()
                    .map(|field| TextDocument::new(field.read(&draft.identity)))
                    .collect(),
                _ => Vec::new(),
            },
            fields: draft.fields.iter().map(writing_from_field).collect(),
            groups: draft
                .groups
                .iter()
                .map(|group| Sheet {
                    name: TextDocument::new(&group.name),
                    fields: group.fields.iter().map(writing_from_field).collect(),
                })
                .collect(),
            recipe: PasswordOptions::new(),
            revealed: false,
            busy: false,
            error: ErrorBanner::default(),
            origin: draft,
        }
    }

    pub(crate) fn is_new(&self) -> bool {
        self.id.is_none()
    }

    pub(crate) fn needs_choices(&self) -> bool {
        self.item_type == ItemType::Alias && self.is_new()
    }

    pub(crate) fn move_to(&mut self, share: ShareId) {
        self.origin.share = share;
    }

    pub(crate) fn took_choices(&mut self, choices: AliasOptions) {
        self.suffix = 0;
        self.mailboxes = choices
            .mailboxes()
            .iter()
            .map(AliasMailbox::id)
            .take(1)
            .collect();
        self.choices = Some(choices);
    }

    pub(crate) fn toggle_mailbox(&mut self, id: i64) {
        match self.mailboxes.iter().position(|mailbox| *mailbox == id) {
            Some(at) => {
                self.mailboxes.remove(at);
            }
            None => self.mailboxes.push(id),
        }
    }

    fn prefix(&self) -> String {
        let prefix = self.prefix.text().trim().to_owned();
        match prefix.is_empty() {
            true => alias_prefix(&self.title.text()),
            false => prefix,
        }
    }

    fn suffix(&self) -> Option<&str> {
        let choices = self.choices.as_ref()?;
        choices
            .suffixes()
            .get(self.suffix)
            .map(|suffix| suffix.suffix())
    }

    pub(crate) fn draft(&self) -> Draft {
        let mut draft = self.origin.clone();
        draft.id = self.id.clone();
        draft.title = self.title.text();
        draft.note = self.note.text();
        draft.email = self.email.text();
        draft.username = self.username.text();
        draft.password = self.password.text();
        draft.totp = self.totp.text();
        draft.urls = self.urls.text();
        draft.card = Card {
            holder: self.holder.text(),
            number: self.number.text(),
            verification: self.verification.text(),
            expiration: self.expiration.text(),
            pin: self.pin.text(),
            kind: self.origin.card.kind,
        };
        draft.wifi = Wifi {
            ssid: self.ssid.text(),
            password: self.wifi_password.text(),
            security: i32::try_from(self.security).unwrap_or_default(),
        };
        draft.ssh = Ssh {
            public: self.public.text(),
            private: self.private.text(),
        };
        for (at, field) in IdentityField::ALL.into_iter().enumerate() {
            if let Some(input) = self.traits.get(at) {
                field.write(&mut draft.identity, input.text());
            }
        }
        draft.fields = self.fields.iter().map(field_from_writing).collect();
        draft.groups = self
            .groups
            .iter()
            .map(|sheet| Group {
                name: sheet.name.text(),
                fields: sheet.fields.iter().map(field_from_writing).collect(),
            })
            .collect();

        draft
    }

    pub(crate) fn alias(&self) -> Result<Option<Made>, Label> {
        if !self.needs_choices() {
            return Ok(None);
        }
        let Some(choices) = self.choices.as_ref() else {
            return Err(tr!("proton.pass.alias-waiting"));
        };
        let Some(suffix) = choices.suffixes().get(self.suffix) else {
            return Err(tr!("proton.pass.alias-waiting"));
        };
        let prefix = self.prefix();
        if prefix.is_empty() {
            return Err(tr!("proton.pass.alias-needs-prefix"));
        }
        if self.mailboxes.is_empty() {
            return Err(tr!("proton.pass.alias-needs-mailbox"));
        }

        Ok(Some(Made {
            prefix,
            suffix: suffix.signed_suffix().to_owned(),
            mailboxes: self.mailboxes.clone(),
        }))
    }

    pub(crate) fn working(&mut self) {
        self.busy = true;
        self.error.clear();
    }

    pub(crate) fn failed(&mut self, failure: Failure) {
        self.busy = false;
        self.error.set(failure);
    }

    pub(crate) fn add_field(&mut self, section: Where) {
        if let Some(fields) = self.writings(section) {
            fields.push(Writing {
                name: TextDocument::new(""),
                value: TextDocument::new(""),
                hidden: false,
            });
        }
    }

    pub(crate) fn drop_field(&mut self, section: Where, at: usize) {
        if let Some(fields) = self.writings(section)
            && at < fields.len()
        {
            fields.remove(at);
        }
    }

    pub(crate) fn hide_field(&mut self, section: Where, at: usize) {
        if let Some(field) = self.writings(section).and_then(|fields| fields.get_mut(at)) {
            field.hidden = !field.hidden;
        }
    }

    pub(crate) fn add_group(&mut self) {
        self.groups.push(Sheet {
            name: TextDocument::new(""),
            fields: Vec::new(),
        });
    }

    pub(crate) fn drop_group(&mut self, at: usize) {
        if at < self.groups.len() {
            self.groups.remove(at);
        }
    }

    fn writings(&mut self, section: Where) -> Option<&mut Vec<Writing>> {
        match section {
            Where::Loose => Some(&mut self.fields),
            Where::In(group) => self.groups.get_mut(group).map(|sheet| &mut sheet.fields),
        }
    }

    pub(crate) fn reveal(&mut self) {
        self.revealed = !self.revealed;
    }

    pub(crate) fn make_password(&mut self) {
        self.password.set_text(generate_password(&self.recipe));
        self.revealed = true;
    }

    pub(crate) fn longer(&mut self, length: usize) {
        self.recipe.length = length;
    }

    pub(crate) fn with_digits(&mut self, digits: bool) {
        self.recipe.digits = digits;
    }

    pub(crate) fn with_symbols(&mut self, symbols: bool) {
        self.recipe.symbols = symbols;
    }

    pub(crate) fn choose_suffix(&mut self, at: usize) {
        self.suffix = at;
    }

    pub(crate) fn choose_security(&mut self, at: usize) {
        self.security = at;
    }
}

pub(crate) fn view(editor: &Editor, vaults: &[Vault]) -> impl View {
    column! {
        pane::toolbar(
            row! {
                icon(list::symbol(editor.item_type)).extent(Length::px(16.0)).color(list::tint(editor.item_type));
                text(match editor.is_new() {
                    true => tr!("proton.pass.new", kind = list::named(editor.item_type).as_str()),
                    false => tr!("proton.pass.editing"),
                })
                .label()
                .semibold()
                .grow(1.0);
                button(text(tr!("proton.pass.save")))
                    .primary()
                    .small()
                    .loading(editor.busy)
                    .on_press(|vault: &mut Pass| vault.keep());
                button(icon(MaterialSymbol::CLOSE).extent(Length::px(15.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(|vault: &mut Pass| vault.close_editor());
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        );
        rule();
        pane::error_banner(&editor.error, |editor: &mut Editor| &mut editor.error);
        scrollable(
            column! {
                if editor.is_new() && vaults.len() > 1 {
                    filed(editor, vaults);
                }
                pane::input(
                    tr!("proton.pass.title"),
                    text_field(&editor.title)
                        .placeholder(tr!("proton.pass.untitled"))
                        .autofocus(true)
                        .fill_width(),
                );
                body(editor);
                pane::input(
                    tr!("proton.pass.note"),
                    pane::prose(&editor.note, 72.0..=220.0),
                );
                fields(editor);
                groups(editor);
            }
            .gap(Space::ROOMY)
            .padding(Space::ROOMY)
            .fill_width(),
        )
        .grow(1.0)
        .fill_width();
    }
    .fill()
}

fn filed(editor: &Editor, vaults: &[Vault]) -> impl View {
    let at = vaults
        .iter()
        .position(|vault| vault.share == editor.origin.share);

    pane::input(
        tr!("proton.pass.vault"),
        combo(at)
            .options(vaults.iter().map(|vault| vault.name.as_str()))
            .on_select(|vault: &mut Pass, at: usize| vault.file_in(at))
            .fill_width(),
    )
}

fn body(editor: &Editor) -> Element {
    match editor.item_type {
        ItemType::Login => login(editor).build(),
        ItemType::Alias => alias(editor),
        ItemType::Card => card(editor).build(),
        ItemType::Wifi => wifi(editor).build(),
        ItemType::SshKey => ssh(editor).build(),
        ItemType::Identity => identity(editor).build(),
        ItemType::Note | ItemType::Custom => Element::empty(),
    }
}

fn identity(editor: &Editor) -> impl View {
    column_of(IdentitySection::ALL.into_iter().map(|facet| {
        column! {
            pane::heading(detail::titled(facet));
            column_of(facet.fields().filter_map(|(at, field)| {
                let input = editor.traits.get(at)?;

                Some(
                    pane::input(
                        detail::field_label(field),
                        text_field(input)
                            .secret(field.is_hidden() && !editor.revealed)
                            .fill_width(),
                    )
                    .build(),
                )
            }))
            .gap(Space::NORMAL)
            .fill_width();
        }
        .gap(Space::NORMAL)
        .fill_width()
        .build()
    }))
    .gap(Space::ROOMY)
    .fill_width()
}

fn login(editor: &Editor) -> impl View {
    column! {
        pane::input(
            tr!("proton.pass.email"),
            text_field(&editor.email)
                .placeholder(tr!("proton.pass.email-hint"))
                .fill_width(),
        );
        pane::input(
            tr!("proton.pass.username"),
            text_field(&editor.username).fill_width(),
        );
        pane::input(
            tr!("proton.pass.password"),
            column! {
                text_field(&editor.password)
                    .secret(!editor.revealed)
                    .trailing(revealer(editor.revealed))
                    .fill_width();
                generator(editor);
            }
            .gap(Space::SNUG)
            .fill_width(),
        );
        pane::input(
            tr!("proton.pass.code-uri"),
            text_field(&editor.totp)
                .placeholder(tr!("proton.pass.code-hint"))
                .fill_width(),
        );
        pane::input(
            tr!("proton.pass.websites"),
            pane::prose(&editor.urls, 40.0..=140.0),
        );
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn revealer(revealed: bool) -> impl View {
    button(
        icon(match revealed {
            true => MaterialSymbol::VISIBILITY_OFF,
            false => MaterialSymbol::VISIBILITY,
        })
        .extent(Length::px(15.0)),
    )
    .ghost()
    .small()
    .icon_only()
    .on_press(|editor: &mut Editor| editor.reveal())
}

fn generator(editor: &Editor) -> impl View {
    row! {
        button(pane::labelled(MaterialSymbol::AUTORENEW, tr!("proton.pass.generate")))
            .secondary()
            .small()
            .on_press(|editor: &mut Editor| editor.make_password());
        text(tr!("proton.pass.length", count = &editor.recipe.length.to_string()))
            .caption()
            .muted()
            .no_wrap();
        slider(editor.recipe.length as f32)
            .range(MIN_PASSWORD_LENGTH as f32..=MAX_PASSWORD_LENGTH as f32)
            .step(1.0)
            .on_change(|editor: &mut Editor, value: f32| editor.longer(value as usize))
            .grow(1.0);
        checkbox(editor.recipe.digits)
            .label(text(tr!("proton.pass.digits")).caption())
            .control_size(ControlSize::Small)
            .on_toggle(|editor: &mut Editor, on: bool| editor.with_digits(on));
        checkbox(editor.recipe.symbols)
            .label(text(tr!("proton.pass.symbols")).caption())
            .control_size(ControlSize::Small)
            .on_toggle(|editor: &mut Editor, on: bool| editor.with_symbols(on));
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn alias(editor: &Editor) -> Element {
    if !editor.is_new() {
        return Element::empty();
    }
    let Some(choices) = editor.choices.as_ref() else {
        return pane::working(tr!("proton.pass.alias-reading"))
            .height(64.0)
            .build();
    };
    let prefix = editor.prefix();
    let suffix = editor.suffix().unwrap_or_default().to_owned();

    column! {
        container(
            row! {
                icon(MaterialSymbol::ALTERNATE_EMAIL)
                    .extent(Length::px(16.0))
                    .color(list::tint(ItemType::Alias));
                text(format!("{prefix}{suffix}"))
                    .label()
                    .semibold()
                    .grow(1.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width(),
        )
        .background(list::tint(ItemType::Alias).with_alpha(0.1))
        .radius(Corner::Panel)
        .padding(Inset::xy(12.0, 10.0))
        .fill_width();
        row! {
            pane::input(
                tr!("proton.pass.alias-prefix"),
                text_field(&editor.prefix)
                    .placeholder(match alias_prefix(&editor.title.text()) {
                        derived if derived.is_empty() => tr!("proton.pass.alias-hint"),
                        derived => Label::from(derived),
                    })
                    .fill_width(),
            )
            .grow(1.0);
            pane::input(
                tr!("proton.pass.alias-domain"),
                combo(editor.suffix)
                    .options(choices.suffixes().iter().map(|suffix| suffix.suffix()))
                    .on_select(|editor: &mut Editor, at: usize| editor.choose_suffix(at))
                    .fill_width(),
            )
            .grow(1.0);
        }
        .gap(Space::NORMAL)
        .fill_width();
        pane::input(
            tr!("proton.pass.alias-mailboxes"),
            column_of(choices.mailboxes().iter().map(|mailbox| {
                let id = mailbox.id();
                checkbox(editor.mailboxes.contains(&id))
                    .label(text(mailbox.email().to_owned()))
                    .on_toggle(move |editor: &mut Editor, _: bool| editor.toggle_mailbox(id))
                    .build()
            }))
            .gap(Space::SNUG)
            .fill_width(),
        );
    }
    .gap(Space::NORMAL)
    .fill_width()
    .build()
}

fn card(editor: &Editor) -> impl View {
    let hidden = !editor.revealed;

    column! {
        pane::input(
            tr!("proton.pass.card-holder"),
            text_field(&editor.holder).fill_width(),
        );
        pane::input(
            tr!("proton.pass.card-number"),
            text_field(&editor.number)
                .secret(hidden)
                .trailing(revealer(editor.revealed))
                .fill_width(),
        );
        row! {
            pane::input(
                tr!("proton.pass.card-expiration"),
                text_field(&editor.expiration)
                    .placeholder(tr!("proton.pass.card-expiration-hint"))
                    .fill_width(),
            )
            .grow(1.0);
            pane::input(
                tr!("proton.pass.card-verification"),
                text_field(&editor.verification).secret(hidden).fill_width(),
            )
            .grow(1.0);
            pane::input(
                tr!("proton.pass.card-pin"),
                text_field(&editor.pin).secret(hidden).fill_width(),
            )
            .grow(1.0);
        }
        .gap(Space::NORMAL)
        .fill_width();
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn wifi(editor: &Editor) -> impl View {
    let kinds: Vec<Label> = (0..=4).map(list::secured).collect();

    column! {
        pane::input(
            tr!("proton.pass.wifi-name"),
            text_field(&editor.ssid).fill_width(),
        );
        pane::input(
            tr!("proton.pass.password"),
            text_field(&editor.wifi_password)
                .secret(!editor.revealed)
                .trailing(revealer(editor.revealed))
                .fill_width(),
        );
        pane::input(
            tr!("proton.pass.wifi-security"),
            combo(editor.security)
                .options(kinds)
                .on_select(|editor: &mut Editor, at: usize| editor.choose_security(at))
                .fill_width(),
        );
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn ssh(editor: &Editor) -> impl View {
    column! {
        pane::input(
            tr!("proton.pass.public-key"),
            pane::code(&editor.public, 48.0..=140.0),
        );
        pane::input(
            tr!("proton.pass.private-key"),
            pane::code(&editor.private, 72.0..=220.0),
        );
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn fields(editor: &Editor) -> impl View {
    writings(editor, Where::Loose, &editor.fields)
}

fn groups(editor: &Editor) -> impl View {
    column! {
        column_of(editor.groups.iter().enumerate().map(|(group, sheet)| {
            column! {
                row! {
                    text_field(&sheet.name)
                        .placeholder(tr!("proton.pass.section-name"))
                        .control_size(ControlSize::Small)
                        .grow(1.0);
                    button(icon(MaterialSymbol::CLOSE).extent(Length::px(14.0)))
                        .ghost()
                        .small()
                        .icon_only()
                        .on_press(move |editor: &mut Editor| editor.drop_group(group));
                }
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .fill_width();
                writings(editor, Where::In(group), &sheet.fields);
            }
            .gap(Space::SNUG)
            .padding(Inset::edges(0.0, 0.0, 0.0, 12.0))
            .fill_width()
            .build()
        }))
        .gap(Space::NORMAL)
        .fill_width();
        button(pane::labelled(MaterialSymbol::ADD, tr!("proton.pass.add-section")))
            .ghost()
            .small()
            .on_press(|editor: &mut Editor| editor.add_group());
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn writings(editor: &Editor, section: Where, fields: &[Writing]) -> impl View {
    let revealed = editor.revealed;

    column! {
        column_of(fields.iter().enumerate().map(|(at, field)| {
            row! {
                text_field(&field.name)
                    .placeholder(tr!("proton.pass.field-name"))
                    .control_size(ControlSize::Small)
                    .width(160.0);
                text_field(&field.value)
                    .placeholder(tr!("proton.pass.field-value"))
                    .control_size(ControlSize::Small)
                    .secret(field.hidden && !revealed)
                    .grow(1.0);
                button(
                    icon(match field.hidden {
                        true => MaterialSymbol::LOCK,
                        false => MaterialSymbol::LOCK_OPEN,
                    })
                    .extent(Length::px(14.0)),
                )
                .ghost()
                .small()
                .icon_only()
                .on_press(move |editor: &mut Editor| editor.hide_field(section, at));
                button(icon(MaterialSymbol::CLOSE).extent(Length::px(14.0)))
                    .ghost()
                    .small()
                    .icon_only()
                    .on_press(move |editor: &mut Editor| editor.drop_field(section, at));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width()
            .build()
        }))
        .gap(Space::SNUG)
        .fill_width();
        button(pane::labelled(MaterialSymbol::ADD, tr!("proton.pass.add-field")))
            .ghost()
            .small()
            .on_press(move |editor: &mut Editor| editor.add_field(section));
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn writing_from_field(field: &CustomField) -> Writing {
    Writing {
        name: TextDocument::new(&field.name),
        value: TextDocument::new(&field.value),
        hidden: field.hidden,
    }
}

fn field_from_writing(field: &Writing) -> CustomField {
    CustomField::new(field.name.text(), field.value.text(), field.hidden)
}
