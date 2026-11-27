use animate_cast::Subject;
use animate_timeline::{READOUT_WIDTH, Readout};
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::animate::{Animate, DockTab};
use crate::command::Command;
use crate::wording;

const GLYPH: f32 = 18.0;
const PICKER_WIDTH: f32 = 200.0;
const VOLUME_WIDTH: f32 = 96.0;
const NOTICE_WIDTH: f32 = 320.0;
const READOUT_HEIGHT: f32 = 28.0;
const DOCK_TABS: [DockTab; 2] = [DockTab::Timeline, DockTab::Nodes];

pub(crate) fn bar(animate: &Animate) -> impl View {
    let transport = animate.transport();
    let removable = match animate.subject() {
        Some(Subject::Figure(_) | Subject::Emitter(_)) => true,
        Some(Subject::Stage | Subject::Accessory(_)) => true,
        Some(Subject::Camera) => animate.has_camera(),
        Some(Subject::Look) => true,
        Some(Subject::Wind) => !animate.cast().wind().motion.is_empty(),
        None => false,
    };
    let clearable = match animate.subject() {
        Some(Subject::Figure(id)) => animate
            .cast()
            .figure(id)
            .is_some_and(|figure| figure.has_motion()),
        Some(subject @ (Subject::Stage | Subject::Accessory(_))) => animate
            .cast()
            .prop(subject)
            .is_some_and(|track| !track.prop.is_still()),
        _ => false,
    };
    let (play, playing) = match transport.is_playing() {
        true => (MaterialSymbol::PAUSE, tr!("animate.command.pause")),
        false => (MaterialSymbol::PLAY_ARROW, tr!("animate.command.play")),
    };
    let register = match animate.has_unregistered() {
        true => ButtonVariant::Primary,
        false => ButtonVariant::Ghost,
    };

    let bar = toolbar()
        .divided()
        .gap(Space::TIGHT)
        .padding(Inset::xy(8.0, 4.0))
        .group(
            tool_group([
                Command::Library
                    .tool(
                        MaterialSymbol::PHOTO_LIBRARY,
                        tr!("animate.command.library"),
                        animate,
                    )
                    .into(),
                subject_picker(animate),
                Command::Add
                    .tool(MaterialSymbol::ADD, tr!("animate.tip.add"), animate)
                    .into(),
                Command::AddLiquid
                    .tool(
                        MaterialSymbol::WATER_DROP,
                        tr!("animate.command.add_liquid"),
                        animate,
                    )
                    .into(),
                tool_button(
                    MaterialSymbol::DELETE_SWEEP,
                    tr!("animate.tip.clear_motion"),
                )
                .enabled(clearable)
                .on_press(|animate: &mut Animate| animate.clear_motion())
                .into(),
                tool_button(MaterialSymbol::CLOSE, tr!("animate.tip.remove_subject"))
                    .enabled(removable)
                    .on_press(|animate: &mut Animate| animate.remove_subject())
                    .into(),
            ])
            .priority(ToolPriority::High),
        )
        .group(
            tool_group([
                Command::Register
                    .tool(
                        MaterialSymbol::ADD_DIAMOND,
                        tr!("animate.command.register"),
                        animate,
                    )
                    .variant(register),
                Command::ResetPose.tool(
                    MaterialSymbol::RESTART_ALT,
                    tr!("animate.command.reset_pose"),
                    animate,
                ),
            ])
            .priority(ToolPriority::High),
        )
        .group(
            tool_group([
                Command::GoToStart
                    .tool(
                        MaterialSymbol::FIRST_PAGE,
                        tr!("animate.command.go_to_start"),
                        animate,
                    )
                    .into(),
                Command::PreviousKey
                    .tool(
                        MaterialSymbol::SKIP_PREVIOUS,
                        tr!("animate.command.previous_key"),
                        animate,
                    )
                    .into(),
                Command::StepBack
                    .tool(
                        MaterialSymbol::CHEVRON_LEFT,
                        tr!("animate.command.step_back"),
                        animate,
                    )
                    .into(),
                Command::PlayPause
                    .tool(play, playing, animate)
                    .variant(ButtonVariant::Primary)
                    .into(),
                Command::StepForward
                    .tool(
                        MaterialSymbol::CHEVRON_RIGHT,
                        tr!("animate.command.step_forward"),
                        animate,
                    )
                    .into(),
                Command::NextKey
                    .tool(
                        MaterialSymbol::SKIP_NEXT,
                        tr!("animate.command.next_key"),
                        animate,
                    )
                    .into(),
                Command::GoToEnd
                    .tool(
                        MaterialSymbol::LAST_PAGE,
                        tr!("animate.command.go_to_end"),
                        animate,
                    )
                    .into(),
                Command::Repeat
                    .tool(
                        MaterialSymbol::REPEAT,
                        tr!("animate.command.repeat"),
                        animate,
                    )
                    .checked(transport.repeats())
                    .into(),
                tool_item(
                    Element::new(Readout::new(transport))
                        .width(READOUT_WIDTH)
                        .height(READOUT_HEIGHT),
                ),
            ])
            .priority(ToolPriority::Pinned),
        )
        .spacer();
    let bar = match animate.notice() {
        Some(notice) => bar.group(tool_group([notice_tool(notice.clone())])),
        None => bar,
    };
    let bar = match transport.volume() {
        Some(volume) => bar.group(sound_tools(volume).priority(ToolPriority::Low)),
        None => bar,
    };
    bar.group(
        tool_group([
            Command::ZoomOut.tool(
                MaterialSymbol::ZOOM_OUT,
                tr!("animate.command.zoom_out"),
                animate,
            ),
            Command::ZoomIn.tool(
                MaterialSymbol::ZOOM_IN,
                tr!("animate.command.zoom_in"),
                animate,
            ),
            Command::ZoomToFit.tool(
                MaterialSymbol::FIT_WIDTH,
                tr!("animate.command.zoom_to_fit"),
                animate,
            ),
        ])
        .priority(ToolPriority::Low),
    )
    .group(dock_tools(animate))
}

fn dock_tools(animate: &Animate) -> ToolGroup {
    let dock = animate.dock();
    let chosen = DOCK_TABS.iter().position(|tab| *tab == dock.tab);
    let labels = [tr!("animate.dock.timeline"), tr!("animate.dock.nodes")];
    let show = |animate: &mut Animate, index: usize| {
        if let Some(tab) = DOCK_TABS.get(index) {
            animate.show_dock(*tab);
        }
    };
    let tabs = segmented(chosen.unwrap_or(0))
        .segments(labels.clone())
        .control_size(ControlSize::Small)
        .focusless()
        .enabled(!dock.split)
        .on_select(show);
    tool_group([
        tool_item(tabs).overflow([menu_item(tr!("animate.dock.panel"))
            .icon(MaterialSymbol::DOCK_TO_BOTTOM)
            .enabled(!dock.split)
            .choices(labels, chosen, show)]),
        Command::SplitDock
            .tool(
                MaterialSymbol::VERTICAL_SPLIT,
                tr!("animate.command.split_dock"),
                animate,
            )
            .checked(dock.split)
            .into(),
    ])
}

fn subject_picker(animate: &Animate) -> ToolItem {
    let cast = animate.cast();
    let subjects = cast.subjects();
    let chosen = animate
        .subject()
        .and_then(|subject| subjects.iter().position(|candidate| *candidate == subject));
    let labels: Vec<Label> = subjects
        .iter()
        .map(|subject| match subject {
            Subject::Figure(id) => cast
                .figure(*id)
                .map_or_else(Label::default, |figure| Label::from(figure.name.as_str())),
            Subject::Emitter(id) => cast
                .emitter(*id)
                .map_or_else(Label::default, |emitter| Label::from(emitter.name.as_str())),
            Subject::Camera => tr!("animate.subject.camera"),
            Subject::Look => tr!("animate.subject.look"),
            Subject::Wind => tr!("animate.subject.wind"),
            Subject::Stage | Subject::Accessory(_) => {
                cast.prop(*subject).map_or_else(Label::default, |track| {
                    wording::prop_subject(track.prop.kind, &track.prop.name)
                })
            }
        })
        .collect();
    let choose = |animate: &mut Animate, chosen: usize| animate.choose_subject(chosen);
    let picker = combo(chosen)
        .options(labels.clone())
        .placeholder(tr!("animate.subject.none"))
        .on_select(choose)
        .focusless()
        .control_size(ControlSize::Small)
        .width(PICKER_WIDTH);
    tool_item(picker).overflow([menu_item(tr!("animate.tip.subject"))
        .icon(MaterialSymbol::CATEGORY)
        .choices(labels, chosen, choose)])
}

fn notice_tool(notice: Label) -> ToolItem {
    let danger = palette().danger;
    let dismiss = |animate: &mut Animate| animate.dismiss_notice();
    let view = row! {
        icon(MaterialSymbol::WARNING)
            .extent(Length::px(GLYPH))
            .color(danger);
        text(notice.clone()).caption().color(danger).no_wrap().shrink(1.0);
        tool_button(MaterialSymbol::CLOSE, tr!("animate.tip.dismiss")).on_press(dismiss);
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .max_width(NOTICE_WIDTH);
    tool_item(view).overflow([menu_item(notice)
        .icon(MaterialSymbol::WARNING)
        .on_press(dismiss)])
}

fn sound_tools(volume: f32) -> ToolGroup {
    let change = |animate: &mut Animate, volume: f32| animate.set_volume(volume);
    let level = row! {
        icon(MaterialSymbol::VOLUME_UP).extent(Length::px(GLYPH));
        slider(volume)
            .range(0.0..=1.0)
            .control_size(ControlSize::Small)
            .on_change(change)
            .width(VOLUME_WIDTH);
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center);
    tool_group([
        tool_item(level).overflow([menu_slider(tr!("animate.tip.volume"), volume, 0.0..=1.0)
            .icon(MaterialSymbol::VOLUME_UP)
            .on_change(change)]),
        tool_button(MaterialSymbol::MUSIC_OFF, tr!("animate.tip.remove_music"))
            .on_press(|animate: &mut Animate| animate.clear_sound())
            .into(),
    ])
}
