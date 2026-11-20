use animate_cast::Mode;
use rikka_app::prelude::*;
use rikka_widgets::row;

use crate::animate::{Animate, LookPart, Starter};
use crate::command::Command;
use crate::wording;

const GLYPH: f32 = 18.0;
const SPINNER: f32 = 16.0;

pub(crate) fn bar(animate: &Animate) -> Option<CaptionBar> {
    let bar = caption().leading(
        row! {
            icon(MaterialSymbol::ANIMATION)
                .extent(Length::px(GLYPH))
                .color(palette().accent);
            menus(animate);
            if animate.is_loading() {
                spinner().extent(Length::px(SPINNER));
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center),
    );
    Some(bar.into())
}

fn menus(animate: &Animate) -> MenuBar {
    let transport = animate.transport();
    let play = match transport.is_playing() {
        true => tr!("animate.command.pause"),
        false => tr!("animate.command.play"),
    };
    let entry = |command: Command, label: Label| command.entry(label, animate);
    let physics = |on: bool| {
        entry(Command::Physics(on), wording::physics(on))
            .selected(animate.is_physics_enabled() == on)
    };
    menu_bar()
        .menu(
            tr!("animate.menu.file"),
            [
                entry(Command::Save, tr!("animate.command.save")),
                entry(Command::SaveCloth, tr!("animate.command.save_cloth")),
                entry(Command::ExportVideo, tr!("animate.command.export_video")),
                menu_separator(),
                entry(Command::Close, tr!("animate.command.close")),
            ],
        )
        .menu(
            tr!("animate.menu.edit"),
            [
                entry(Command::Undo, tr!("animate.command.undo")),
                entry(Command::Redo, tr!("animate.command.redo")),
                menu_separator(),
                entry(Command::Cut, tr!("animate.command.cut")),
                entry(Command::Copy, tr!("animate.command.copy")),
                entry(Command::Paste, tr!("animate.command.paste")),
                entry(Command::DeleteKeys, tr!("animate.command.delete_keys")),
                menu_separator(),
                entry(Command::Register, tr!("animate.command.register")),
                entry(Command::ResetPose, tr!("animate.command.reset_pose")),
                entry(Command::Pin, tr!("animate.command.pin")),
                entry(Command::ReleaseHold, tr!("animate.command.release_hold")),
                menu_separator(),
                entry(Command::AddLiquid, tr!("animate.command.add_liquid")),
                entry(
                    Command::DuplicateLiquid,
                    tr!("animate.command.duplicate_liquid"),
                ),
                menu_separator(),
                entry(Command::SwitchMode, tr!("animate.command.easy_mode"))
                    .selected(animate.mode() == Mode::Easy),
            ],
        )
        .menu(
            tr!("animate.menu.view"),
            [
                entry(Command::FrameEveryone, tr!("animate.command.frame_all")),
                entry(Command::RestView, tr!("animate.command.rest_view")),
                menu_separator(),
                entry(Command::OutputSize, tr!("animate.command.output_size")),
                menu_separator(),
                entry(Command::Inspector, tr!("animate.command.inspector"))
                    .selected(animate.is_inspector_open()),
                entry(Command::Assistant, tr!("animate.command.assistant")),
                entry(Command::NodeDock, tr!("animate.command.node_dock"))
                    .selected(animate.dock().shows_nodes()),
                entry(Command::SplitDock, tr!("animate.command.split_dock"))
                    .selected(animate.dock().split),
                menu_separator(),
                entry(Command::ZoomIn, tr!("animate.command.zoom_in")),
                entry(Command::ZoomOut, tr!("animate.command.zoom_out")),
                entry(Command::ZoomToFit, tr!("animate.command.zoom_to_fit")),
            ],
        )
        .menu(
            tr!("animate.menu.look"),
            [
                entry(
                    Command::ResetLook(LookPart::World, Starter::Bare),
                    tr!("animate.command.bare_world"),
                ),
                entry(
                    Command::ResetLook(LookPart::World, Starter::Cinematic),
                    tr!("animate.command.cinematic_world"),
                ),
                menu_separator(),
                entry(
                    Command::ResetLook(LookPart::Composite, Starter::Bare),
                    tr!("animate.command.bare_composite"),
                ),
                entry(
                    Command::ResetLook(LookPart::Composite, Starter::Cinematic),
                    tr!("animate.command.cinematic_composite"),
                ),
            ],
        )
        .menu(
            tr!("animate.menu.playback"),
            [
                entry(Command::PlayPause, play),
                entry(Command::Repeat, tr!("animate.command.repeat")).selected(transport.repeats()),
                menu_separator(),
                entry(Command::StepBack, tr!("animate.command.step_back")),
                entry(Command::StepForward, tr!("animate.command.step_forward")),
                entry(Command::LeapBack, tr!("animate.command.leap_back")),
                entry(Command::LeapForward, tr!("animate.command.leap_forward")),
                menu_separator(),
                entry(Command::PreviousKey, tr!("animate.command.previous_key")),
                entry(Command::NextKey, tr!("animate.command.next_key")),
                entry(Command::GoToStart, tr!("animate.command.go_to_start")),
                entry(Command::GoToEnd, tr!("animate.command.go_to_end")),
            ],
        )
        .menu(
            tr!("animate.menu.physics"),
            [
                physics(false),
                physics(true),
                menu_separator(),
                entry(Command::ResetPhysics, tr!("animate.command.reset_physics")),
                menu_separator(),
                entry(
                    Command::Liquids(!animate.pours()),
                    tr!("animate.command.compute_liquids"),
                )
                .selected(animate.pours()),
                entry(
                    Command::RecomputeLiquids,
                    tr!("animate.command.recompute_liquids"),
                ),
            ],
        )
}
