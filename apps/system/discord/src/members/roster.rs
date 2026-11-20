use std::collections::HashMap;
use std::ops::Range;

use discord_core::{Channel, ChannelId, Picture, RosterGroup, RosterRow, SpaceId, Status, UserId};
use rikka_app::prelude::*;
use rikka_ui::Handler;
use rikka_widgets::{column, row};

use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::widgets::{avatar_with_status, context_area, mark_of};

const GROUP: f32 = 40.0;
const MEMBER: f32 = 44.0;
const FACE: f32 = 32.0;

#[derive(Clone, Debug)]
enum Line {
    Group { label: String, count: u32 },
    Member(MemberLine),
    Unknown,
}

#[derive(Clone, Debug)]
struct MemberLine {
    key: String,
    user: UserId,
    space: Option<SpaceId>,
    name: String,
    color: Option<Color>,
    picture: Option<Picture>,
    status: Status,
    activity: Option<String>,
}

pub(crate) fn view(linked: &LinkedAccount, channel: &Channel) -> Element {
    let Some(roster) = linked.roster(&channel.id) else {
        return container(spinner()).center().fill().any();
    };
    let space = channel.space.as_ref();
    let known: HashMap<usize, Line> = roster
        .known
        .iter()
        .map(|(&at, row)| {
            let shown = match row {
                RosterRow::Group { label, count } => Line::Group {
                    label: group_label(linked, space, label),
                    count: *count,
                },
                RosterRow::Member(user) => {
                    member_line(linked, space, user).map_or(Line::Unknown, Line::Member)
                }
            };
            (at as usize, shown)
        })
        .collect();
    let channel_id = channel.id;
    lazy_column(roster.len as usize, move |slot| {
        line(known.get(&slot.index).unwrap_or(&Line::Unknown))
    })
    .padding(Inset::xy(Space::SNUG, Space::SNUG))
    .shows((linked.key().to_owned(), channel.id))
    .on_visible(move |chat: &mut Chat, shown: Range<usize>| chat.view_roster(&channel_id, shown))
    .fill()
    .any()
}

pub(crate) fn recipients(linked: &LinkedAccount, channel: &Channel) -> Element {
    let mut users: Vec<UserId> = channel.recipients.clone();
    if let Some(me) = linked.me() {
        users.push(me.id);
    }
    let mut lines: Vec<Line> = vec![Line::Group {
        label: tr!("discord.members.members").to_string(),
        count: users.len() as u32,
    }];
    lines.extend(
        users
            .iter()
            .filter_map(|user| member_line(linked, None, user))
            .map(Line::Member),
    );
    lazy_column(lines.len(), move |slot| line(&lines[slot.index]))
        .padding(Inset::xy(Space::SNUG, Space::SNUG))
        .shows((linked.key().to_owned(), channel.id))
        .fill()
        .any()
}

fn group_label(linked: &LinkedAccount, space: Option<&SpaceId>, group: &RosterGroup) -> String {
    match group {
        RosterGroup::Role(role) => space
            .and_then(|space| linked.role(space, role))
            .map_or_else(|| role.to_string(), |role| role.name.clone()),
        RosterGroup::Online => tr!("discord.status.online").to_string(),
        RosterGroup::Offline => tr!("discord.status.offline").to_string(),
    }
    .to_uppercase()
}

fn member_line(
    linked: &LinkedAccount,
    space: Option<&SpaceId>,
    user: &UserId,
) -> Option<MemberLine> {
    let known = linked.user(user)?;
    let presence = linked.presence(user);
    let picture = space
        .and_then(|space| linked.member(space, user))
        .and_then(|member| member.avatar.clone())
        .or_else(|| known.avatar.clone());
    Some(MemberLine {
        key: linked.key().to_owned(),
        user: *user,
        space: space.cloned(),
        name: linked.display_name(space, known),
        color: linked.color_of(space, user).map(Color::hex),
        picture,
        status: presence.map_or(Status::Offline, |presence| presence.status),
        activity: presence.and_then(|presence| {
            presence
                .mood
                .clone()
                .filter(|mood| !mood.is_empty())
                .or_else(|| {
                    presence
                        .activity
                        .as_ref()
                        .map(|activity| activity.name.clone())
                })
        }),
    })
}

fn line(line: &Line) -> Element {
    match line {
        Line::Group { label, count } => container(
            text(format!("{label} — {count}"))
                .caption()
                .semibold()
                .muted()
                .no_wrap(),
        )
        .padding(Inset::edges(
            Space::ROOMY,
            Space::SNUG,
            Space::TIGHT,
            Space::SNUG,
        ))
        .align_y(Align::End)
        .height(GROUP)
        .fill_width()
        .any(),
        Line::Member(member) => member_row(member),
        Line::Unknown => container(
            container(spacer())
                .background(palette().surface_variant)
                .radius(Corner::Detail)
                .size(Length::px(120.0), Length::px(12.0)),
        )
        .padding(Inset::xy(Space::SNUG, Space::NONE))
        .align_y(Align::Center)
        .height(MEMBER)
        .fill_width()
        .any(),
    }
}

fn member_row(member: &MemberLine) -> Element {
    let mark = mark_of(("discord-member", &member.key, &member.user));
    let away = !member.status.is_present();
    let (shown, menu_member) = (member.clone(), member.clone());
    let body = container(
        row! {
            avatar_with_status(
                member.picture.as_ref(),
                &member.name,
                FACE,
                member.status,
                palette().surface,
            );
            column! {
                text(member.name.clone())
                    .label()
                    .medium()
                    .no_wrap()
                    .color(member.color.unwrap_or(palette().text));
                if let Some(activity) = &member.activity {
                    text(activity.clone()).caption().muted().no_wrap();
                }
            }
            .grow(1.0)
            .min_width(0.0);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .radius(Corner::Item)
    .on_press(move |chat: &mut Chat| {
        chat.open_profile(
            &shown.user,
            shown.space.as_ref(),
            Anchor::new(mark, AnchorEdge::BesideLeftTop).offset(-Space::SNUG.px(), 0.0),
        );
    })
    .focusless()
    .height(MEMBER)
    .fill_width()
    .opacity(if away { 0.5 } else { 1.0 });
    context_area(
        mark,
        body,
        Handler::new(move |chat: &mut Chat, anchor: Anchor| {
            chat.open_member_menu(&menu_member.user, menu_member.space.as_ref(), anchor);
        }),
    )
}

impl Chat {
    pub(crate) fn view_roster(&mut self, channel: &ChannelId, shown: Range<usize>) {
        skip_view();
        let (Ok(start), Ok(end)) = (u32::try_from(shown.start), u32::try_from(shown.end)) else {
            return;
        };
        if let Some(linked) = self.current() {
            linked.client().view_roster(channel, start..end);
        }
    }
}
