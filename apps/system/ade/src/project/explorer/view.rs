use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ade_files::Kind;
use ade_git::State;
use ade_tree::Row;
use rikka_app::prelude::{Event as Input, *};
use rikka_clipboard::Offer;
use rikka_open::path_to_uri;
use rikka_ui::{
    Accept, DRAG_SLOP, DragAction, DragActions, DragOffer, DragStart, URI_LIST, hash_of,
};
use rikka_widgets::{Surface, column, drag_chip, row};

use crate::hint::hint;
use crate::labels;
use crate::notice::notice;
use crate::project::act::{act, given};
use crate::project::explorer::actions::{self, ExplorerAction};
use crate::project::explorer::clipboard;
use crate::project::explorer::jobs;
use crate::project::explorer::state::{Explorer, Gesture, Naming, Pressed, Summoned};
use crate::project::state::Project;
use crate::project::workspace::Workspace;
use crate::project::{git, glyph};

const ROW: f32 = 24.0;
const INDENT: f32 = 12.0;
const GLYPH: f32 = 16.0;
const FONT: f32 = 13.0;
const TOOL: f32 = 16.0;
const MARK: &str = "ade-explorer-mark";
const MENU: &str = "ade-explorer-menu";

#[derive(Clone, Debug)]
enum Line {
    Row {
        row: Row,
        state: Option<State>,
        unsaved: bool,
        chosen: bool,
        aimed: bool,
    },
    Renaming {
        row: Row,
    },
    Draft {
        depth: usize,
        kind: Kind,
    },
}

impl Line {
    fn row(&self) -> Option<&Row> {
        match self {
            Line::Row { row, .. } | Line::Renaming { row } => Some(row),
            Line::Draft { .. } => None,
        }
    }

    fn folder(&self) -> Option<PathBuf> {
        let row = self.row()?;
        Some(match row.is_folder() {
            true => row.path().clone(),
            false => row.path().parent()?.to_path_buf(),
        })
    }
}

pub(in crate::project) fn view(workspace: &Workspace, place: &Place) -> Element {
    let explorer = workspace.explorer();
    column! {
        header(workspace.root());
        for running in explorer.jobs() {
            jobs::view(running);
        }
        if let Some(error_message) = explorer.error_message() {
            container(
                notice(MaterialSymbol::ERROR, error_message, palette().danger)
                    .on_dismiss(tr!("ade.files.dismiss"), act(|project| {
                        project.explore(|explorer, _| explorer.forget_trouble());
                    })),
            )
            .padding(Inset::xy(0.0, 4.0))
            .fill_width();
        }
        if let Some(erasing) = explorer.erasing() {
            erasing_asked(erasing);
        }
        spacer().width(0.0).height(0.0).build().keyed(MARK);
        tree(workspace, place);
        if let Some(summoned) = explorer.menu() {
            menu()
                .items(menu_items(explorer, summoned))
                .width(248.0)
                .on_dismiss(act(|project| project.explore(|explorer, _| explorer.dismiss())))
                .build()
                .keyed((MENU, summoned.key))
                .anchored(Anchor::new(MARK, AnchorEdge::BesideTop).offset(summoned.at.x, summoned.at.y));
        }
    }
    .grow(1.0)
    .fill_width()
    .any()
}

fn header(root: &Path) -> impl View {
    row! {
        text(labels::file_name(root).to_uppercase())
            .caption()
            .medium()
            .muted()
            .no_wrap()
            .grow(1.0);
        tool(MaterialSymbol::NOTE_ADD, ExplorerAction::NewFile);
        tool(MaterialSymbol::CREATE_NEW_FOLDER, ExplorerAction::NewFolder);
        tool(MaterialSymbol::UNFOLD_LESS, ExplorerAction::CollapseAll);
    }
    .gap(Space::NONE)
    .align_items(Align::Center)
    .padding(Inset::exact(0.0, 0.0, 2.0, 6.0))
    .fill_width()
}

fn tool(symbol: MaterialSymbol, action: ExplorerAction) -> impl View {
    button(icon(symbol).extent(Length::px(TOOL)))
        .icon_only()
        .ghost()
        .small()
        .on_press(perform(action))
}

fn perform(action: ExplorerAction) -> impl Fn(&mut Project) + 'static {
    act(move |project| project.explore(|explorer, parts| actions::perform(explorer, parts, action)))
}

fn erasing_asked(erasing: &[PathBuf]) -> impl View {
    let message = match erasing {
        [one] => tr!("ade.files.erase-one", name = &labels::file_name(one)),
        many => tr!("ade.files.erase-many", count = many.len() as i64),
    };
    container(
        column! {
            text(message).caption().color(palette().danger);
            row! {
                button(text(tr!("ade.files.erase")).caption())
                    .small()
                    .on_press(act(|project| project.explore(actions::delete_confirmed)));
                button(text(tr!("ade.files.keep")).caption())
                    .secondary()
                    .small()
                    .on_press(act(|project| {
                        project.explore(|explorer, _| drop(explorer.take_erasing()));
                    }));
            }
            .gap(Space::SNUG);
        }
        .gap(Space::SNUG),
    )
    .padding(Inset::xy(6.0, 6.0))
    .fill_width()
}

fn lines(workspace: &Workspace) -> Vec<Line> {
    let explorer = workspace.explorer();
    let tree = workspace.tree();
    let changed = workspace.git().changed();
    let viewer = workspace.viewer();
    let rows = tree.rows();
    let renaming = match explorer.naming() {
        Some(Naming::Rename { path, .. }) => Some(path.clone()),
        _ => None,
    };
    let draft = match explorer.naming() {
        Some(Naming::Create { folder, kind, .. }) if folder.as_path() == tree.root() => {
            Some((0, 0, *kind))
        }
        Some(Naming::Create { folder, kind, .. }) => rows
            .iter()
            .position(|row| row.path() == folder)
            .map(|at| (at + 1, rows[at].depth() + 1, *kind)),
        _ => None,
    };
    let aim = explorer.aim();
    let mut lines: Vec<Line> = rows
        .into_iter()
        .map(|row| {
            if renaming.as_deref() == Some(row.path().as_path()) {
                return Line::Renaming { row };
            }
            let state = match row.is_folder() {
                true => changed.folder(row.path()),
                false => changed.file(row.path()),
            };
            Line::Row {
                unsaved: viewer.is_unsaved(row.path()),
                chosen: explorer.selection().contains(row.path()),
                aimed: aim == Some(row.path().as_path()),
                state,
                row,
            }
        })
        .collect();
    if let Some((at, depth, kind)) = draft {
        lines.insert(at.min(lines.len()), Line::Draft { depth, kind });
    }
    lines
}

fn tree(workspace: &Workspace, place: &Place) -> Element {
    let explorer = workspace.explorer();
    let lines: Rc<[Line]> = lines(workspace).into();
    if lines.is_empty() {
        return match workspace.tree().is_read() {
            true => hint(tr!("ade.sidebar.no-files")).any(),
            false => hint(tr!("ade.sidebar.reading")).any(),
        };
    }
    let focus = explorer.selection().focus().and_then(|focus| {
        lines
            .iter()
            .position(|line| line.row().is_some_and(|row| row.path() == focus))
    });
    let reveals = lines
        .iter()
        .position(|line| matches!(line, Line::Draft { .. } | Line::Renaming { .. }))
        .or_else(|| {
            let revealing = explorer.revealing()?;
            lines
                .iter()
                .position(|line| line.row().is_some_and(|row| row.path() == revealing))
        });
    let naming = explorer.naming().map(|naming| naming.document().clone());
    let has_error = explorer.error_message().is_some();
    let root = workspace.root().to_owned();
    let pointing = Pointing::new(explorer, Rc::clone(&lines), root);
    let walking = Rc::clone(&lines);

    let drawn = Rc::clone(&lines);
    lazy_column(lines.len(), move |slot| {
        line_view(&drawn[slot.index], naming.as_ref(), has_error)
    })
    .uniform(ROW)
    .named({
        let named = Rc::clone(&lines);
        move |index| named.get(index).map_or(0, name_of)
    })
    .focus(focus)
    .reveals(reveals)
    .autofocus(false)
    .lit(true)
    .place(place)
    .on_focus(
        move |project: &mut Project, index: usize, modifiers: Modifiers| {
            if let Some(row) = walking.get(index).and_then(Line::row) {
                let path = row.path().clone();
                project.explore(|explorer, parts| {
                    actions::extend_selection(explorer, parts, &path, modifiers.shift)
                });
            }
            project.settle();
        },
    )
    .on_pointer(move |event, ctx, surface| pointing.event(event, ctx, surface))
    .grow(1.0)
    .fill_width()
    .any()
}

fn name_of(line: &Line) -> u64 {
    match line {
        Line::Row { row, .. } | Line::Renaming { row } => hash_of(row.path()),
        Line::Draft { depth, .. } => hash_of(("draft", depth)),
    }
}

fn line_view(line: &Line, naming: Option<&TextDocument>, has_error: bool) -> Element {
    match line {
        Line::Row {
            row,
            state,
            unsaved,
            chosen,
            aimed,
        } => row_view(row, *state, *unsaved, *chosen, *aimed),
        Line::Renaming { row } => match naming {
            Some(document) => naming_view(
                row.depth(),
                symbol_of(row),
                tint_of(row),
                document,
                has_error,
            ),
            None => Element::empty(),
        },
        Line::Draft { depth, kind } => match naming {
            Some(document) => {
                let symbol = match kind {
                    Kind::File => MaterialSymbol::DRAFT,
                    Kind::Folder => glyph::folder(false),
                };
                let tint = match kind {
                    Kind::File => palette().text_muted,
                    Kind::Folder => glyph::folder_tint(),
                };
                naming_view(*depth, symbol, tint, document, has_error)
            }
            None => Element::empty(),
        },
    }
}

fn symbol_of(row: &Row) -> MaterialSymbol {
    match row.is_folder() {
        true => glyph::folder(row.is_open()),
        false => glyph::file(row.path()),
    }
}

fn tint_of(row: &Row) -> Color {
    match row.is_folder() {
        true => glyph::folder_tint(),
        false => palette().text_muted,
    }
}

fn indent(depth: usize) -> impl View {
    let guide = palette().outline.scale_alpha(0.6);
    row! {
        for _ in 0..depth {
            row! {
                container(spacer()).background(guide).width(1.0).fill_height();
                spacer().width(INDENT - 1.0);
            }
            .fill_height();
        }
    }
    .padding(Inset::exact(0.0, 0.0, 0.0, GLYPH / 2.0))
    .fill_height()
}

fn row_view(row: &Row, state: Option<State>, unsaved: bool, chosen: bool, aimed: bool) -> Element {
    let palette = palette();
    let ink = match (chosen, state) {
        (true, _) => palette.on_selection,
        (false, Some(state)) => git::ink(state, palette),
        (false, None) => palette.text,
    };
    let background = match (chosen, aimed) {
        (_, true) => palette.accent.scale_alpha(0.22),
        (true, false) => palette.selection,
        (false, false) => Color::TRANSPARENT,
    };
    container(
        row! {
            indent(row.depth());
            match row.is_folder() {
                true => twistie(row.is_open(), GLYPH, palette.text_muted),
                false => spacer().width(GLYPH).any(),
            };
            icon(symbol_of(row))
                .extent(Length::px(GLYPH))
                .color(tint_of(row));
            text(row.name())
                .text_size(FONT)
                .color(ink)
                .no_wrap()
                .grow(1.0);
            if unsaved {
                icon(MaterialSymbol::CIRCLE)
                    .extent(Length::px(8.0))
                    .color(palette.text);
            }
            if let Some(state) = state.filter(|_| !row.is_folder()) {
                text(letter(state))
                    .text_size(FONT - 1.0)
                    .medium()
                    .color(git::ink(state, palette))
                    .no_wrap();
            }
        }
        .gap(4.0)
        .align_items(Align::Center)
        .padding(Inset::exact(0.0, 8.0, 0.0, 2.0))
        .fill(),
    )
    .background(background)
    .radius(Corner::Detail)
    .height(ROW)
    .fill_width()
    .any()
}

fn naming_view(
    depth: usize,
    symbol: MaterialSymbol,
    tint: Color,
    document: &TextDocument,
    has_error: bool,
) -> Element {
    row! {
        indent(depth);
        spacer().width(GLYPH);
        icon(symbol).extent(Length::px(GLYPH)).color(tint);
        text_field(document)
            .control_size(ControlSize::Small)
            .font_size(FONT)
            .error(has_error)
            .autofocus(true)
            .claims_focus(true)
            .on_change(act(|project| project.explore(|explorer, _| actions::name_changed(explorer))))
            .on_submit(act(|project| project.explore(actions::submit_name)))
            .on_blur(act(|project| project.explore(actions::submit_name)))
            .on_key(|project: &mut Project, key: Key, _: Modifiers| {
                if key != Key::Escape {
                    return false;
                }
                project.explore(|explorer, _| drop(explorer.take_naming()));
                project.settle();
                true
            })
            .height(ROW)
            .grow(1.0);
    }
    .gap(4.0)
    .align_items(Align::Center)
    .padding(Inset::exact(0.0, 4.0, 0.0, 2.0))
    .height(ROW)
    .fill_width()
    .any()
}

fn letter(state: State) -> &'static str {
    match state {
        State::Added => "A",
        State::Modified | State::Retyped => "M",
        State::Deleted => "D",
        State::Renamed => "R",
        State::Copied => "C",
        State::Untracked => "U",
        State::Conflicted => "!",
    }
}

fn menu_items(explorer: &Explorer, summoned: Summoned) -> Vec<MenuItem> {
    let on_row = summoned.on_row;
    let chosen = explorer.selection().len();
    let one = chosen == 1;
    let any = chosen > 0;
    let file = one && summoned.on_file;
    let item = |label: Label, symbol: MaterialSymbol, action: ExplorerAction| {
        menu_item(label).icon(symbol).on_press(perform(action))
    };
    let mut items = Vec::new();
    if on_row && file {
        items.push(item(
            tr!("ade.files.open"),
            MaterialSymbol::OPEN_IN_NEW,
            ExplorerAction::Open,
        ));
        items.push(menu_separator());
    }
    items.push(item(
        tr!("ade.files.new-file"),
        MaterialSymbol::NOTE_ADD,
        ExplorerAction::NewFile,
    ));
    items.push(item(
        tr!("ade.files.new-folder"),
        MaterialSymbol::CREATE_NEW_FOLDER,
        ExplorerAction::NewFolder,
    ));
    items.push(item(
        tr!("ade.files.reveal"),
        MaterialSymbol::FOLDER_OPEN,
        ExplorerAction::Reveal,
    ));
    items.push(menu_separator());
    if on_row {
        items.push(
            item(
                tr!("ade.files.cut"),
                MaterialSymbol::CONTENT_CUT,
                ExplorerAction::Cut,
            )
            .shortcut("Ctrl+X")
            .enabled(any),
        );
        items.push(
            item(
                tr!("ade.files.copy"),
                MaterialSymbol::CONTENT_COPY,
                ExplorerAction::Copy,
            )
            .shortcut("Ctrl+C")
            .enabled(any),
        );
    }
    items.push(
        item(
            tr!("ade.files.paste"),
            MaterialSymbol::CONTENT_PASTE,
            ExplorerAction::Paste,
        )
        .shortcut("Ctrl+V")
        .enabled(summoned.pasteable),
    );
    if on_row {
        items.push(
            item(
                tr!("ade.files.duplicate"),
                MaterialSymbol::FILE_COPY,
                ExplorerAction::Duplicate,
            )
            .shortcut("Ctrl+D")
            .enabled(any),
        );
        items.push(menu_separator());
        items.push(
            item(
                tr!("ade.files.copy-path"),
                MaterialSymbol::LINK,
                ExplorerAction::CopyPath,
            )
            .shortcut("Ctrl+Alt+C")
            .enabled(any),
        );
        items.push(
            item(
                tr!("ade.files.copy-relative-path"),
                MaterialSymbol::LINK,
                ExplorerAction::CopyRelativePath,
            )
            .shortcut("Ctrl+Shift+Alt+C")
            .enabled(any),
        );
        items.push(menu_separator());
        items.push(
            item(
                tr!("ade.files.rename"),
                MaterialSymbol::DRIVE_FILE_RENAME_OUTLINE,
                ExplorerAction::Rename,
            )
            .shortcut("F2")
            .enabled(one),
        );
        items.push(
            item(
                tr!("ade.files.trash"),
                MaterialSymbol::DELETE,
                ExplorerAction::Trash,
            )
            .shortcut("Delete")
            .enabled(any),
        );
        items.push(
            item(
                tr!("ade.files.erase-forever"),
                MaterialSymbol::DELETE_FOREVER,
                ExplorerAction::Erase,
            )
            .shortcut("Shift+Delete")
            .enabled(any),
        );
    } else {
        items.push(menu_separator());
        items.push(item(
            tr!("ade.files.collapse"),
            MaterialSymbol::UNFOLD_LESS,
            ExplorerAction::CollapseAll,
        ));
    }
    items
}

struct Pointing {
    lines: Rc<[Line]>,
    gesture: Rc<RefCell<Gesture>>,
    chosen: Rc<Vec<PathBuf>>,
    root: PathBuf,
    press: Handler<((usize, Modifiers, u8),)>,
    summon: Handler<((Option<usize>, Point),)>,
    acted: Handler<(ExplorerAction,)>,
    stepped: Handler<(bool,)>,
    cleared: Handler,
    aimed: Handler<(Option<PathBuf>,)>,
    dropped: Handler<((DragOffer, PathBuf, bool),)>,
}

impl Pointing {
    fn new(explorer: &Explorer, lines: Rc<[Line]>, root: PathBuf) -> Pointing {
        let pressing = Rc::clone(&lines);
        let summoning = Rc::clone(&lines);
        let land = root.clone();
        Pointing {
            gesture: explorer.gesture(),
            chosen: Rc::new(explorer.selection().topmost()),
            press: Handler::new(given(
                move |project, (index, modifiers, presses): (usize, Modifiers, u8)| {
                    let Some(row) = pressing.get(index).and_then(Line::row).cloned() else {
                        skip_view();
                        return;
                    };
                    project.explore(|explorer, parts| {
                        actions::pressed(explorer, parts, &row, modifiers, presses);
                    });
                },
            )),
            summon: Handler::new(given(
                move |project, (index, at): (Option<usize>, Point)| {
                    let row = index
                        .and_then(|index| summoning.get(index))
                        .and_then(Line::row)
                        .cloned();
                    project.explore(|explorer, _| {
                        actions::open_context_menu(explorer, row.as_ref(), at)
                    });
                },
            )),
            acted: Handler::new(given(|project, action: ExplorerAction| {
                project.explore(|explorer, parts| actions::perform(explorer, parts, action));
            })),
            stepped: Handler::new(given(|project, into: bool| {
                project.explore(|explorer, parts| {
                    if !actions::navigate(explorer, parts, into) {
                        skip_view();
                    }
                });
            })),
            cleared: Handler::new(act(|project| {
                project.explore(|explorer, _| {
                    explorer.dismiss_quietly();
                    explorer.chosen().clear();
                });
            })),
            aimed: Handler::new(given(|project, aim: Option<PathBuf>| {
                project.explore(|explorer, _| explorer.aim_at(aim));
            })),
            dropped: Handler::new(given(
                move |_, (offer, folder, moving): (DragOffer, PathBuf, bool)| {
                    let root = land.clone();
                    spawn_blocking(
                        move || clipboard::paths_of(&offer.uris()),
                        given(move |project, sources: Vec<PathBuf>| {
                            let (folder, root) = (folder.clone(), root.clone());
                            match project.workspace(&root) {
                                Some(workspace) => workspace.explore(|explorer, parts| {
                                    actions::drop_paths(explorer, parts, sources, folder, moving);
                                }),
                                None => skip_view(),
                            }
                        }),
                    );
                },
            )),
            lines,
            root,
        }
    }

    fn event(&self, event: &Input, ctx: &mut EventCtx<'_>, surface: &Surface<'_>) -> bool {
        match *event {
            Input::PointerPressed {
                button,
                position,
                modifiers,
            } if ctx.bounds().contains(position) => {
                self.pressed(button, position, modifiers, ctx, surface)
            }
            Input::PointerMoved(position) => self.pulled(position, ctx),
            Input::PointerReleased {
                button: PointerButton::Primary,
                ..
            } => {
                if self.gesture.borrow_mut().pressed.take().is_none() {
                    return false;
                }
                ctx.release_pointer();
                ctx.set_handled();
                true
            }
            Input::KeyPressed { key, modifiers, .. } if ctx.focused() => {
                self.keyed(key, modifiers, ctx)
            }
            Input::Command(command) if ctx.focused() => self.commanded(command, ctx),
            Input::DragMoved {
                position,
                ref offer,
            } => self.aiming(position, offer, ctx, surface),
            Input::DragLeft => {
                if self.gesture.borrow_mut().aim.take().is_some() {
                    ctx.call(&self.aimed, (None,));
                }
                false
            }
            Input::DragDropped {
                ref offer, action, ..
            } => {
                let Some(folder) = self.gesture.borrow_mut().aim.take() else {
                    return false;
                };
                ctx.set_handled();
                ctx.call(
                    &self.dropped,
                    ((offer.clone(), folder, action == DragAction::Move),),
                );
                true
            }
            _ => false,
        }
    }

    fn pressed(
        &self,
        button: PointerButton,
        position: Point,
        modifiers: Modifiers,
        ctx: &mut EventCtx<'_>,
        surface: &Surface<'_>,
    ) -> bool {
        let index = surface.at(position).filter(|index| {
            self.lines
                .get(*index)
                .is_some_and(|line| line.row().is_some())
        });
        ctx.request_focus();
        ctx.set_handled();
        match button {
            PointerButton::Primary => {
                let Some(index) = index else {
                    ctx.call(&self.cleared, ());
                    return true;
                };
                self.gesture.borrow_mut().pressed = Some(Pressed {
                    index,
                    at: position,
                });
                ctx.capture_pointer();
                let presses = ctx.presses().min(2) as u8;
                ctx.call(&self.press, ((index, modifiers, presses),));
            }
            PointerButton::Secondary => {
                let origin = ctx.bounds().origin;
                let at = Point::new(position.x - origin.x, position.y - origin.y);
                ctx.call(&self.summon, ((index, at),));
            }
            _ => return false,
        }
        true
    }

    fn pulled(&self, position: Point, ctx: &mut EventCtx<'_>) -> bool {
        let pressed = self.gesture.borrow().pressed;
        let Some(pressed) = pressed else {
            return false;
        };
        ctx.set_handled();
        if position.distance(pressed.at) <= DRAG_SLOP {
            return true;
        }
        self.gesture.borrow_mut().pressed = None;
        ctx.release_pointer();
        self.lift(pressed.index, ctx);
        true
    }

    fn lift(&self, index: usize, ctx: &mut EventCtx<'_>) {
        let Some(row) = self.lines.get(index).and_then(Line::row) else {
            return;
        };
        let files: Vec<PathBuf> = match self.chosen.contains(row.path()) {
            true => (*self.chosen).clone(),
            false => vec![row.path().clone()],
        };
        let uris: Vec<String> = files.iter().map(|path| path_to_uri(path)).collect();
        let offer = Offer::uris(&uris).with_text(clipboard::paths_text(&files, None));
        let label = match files.len() {
            1 => row.name().to_owned(),
            count => format!("{} +{}", row.name(), count - 1),
        };
        let icon = drag_chip(ctx.theme().clone(), symbol_of(row), &label);
        ctx.start_drag(DragStart::new(offer, icon).actions(DragActions::COPY_OR_MOVE));
    }

    fn aiming(
        &self,
        position: Point,
        offer: &DragOffer,
        ctx: &mut EventCtx<'_>,
        surface: &Surface<'_>,
    ) -> bool {
        if !offer.has_uris() || !ctx.bounds().contains(position) {
            return false;
        }
        let folder = surface
            .at(position)
            .and_then(|index| self.lines.get(index))
            .map_or_else(|| Some(self.root.clone()), Line::folder);
        let files = offer.is_ours()
            && folder
                .as_ref()
                .is_some_and(|folder| self.chosen.iter().any(|path| folder.starts_with(path)));
        let aim = folder.filter(|_| !files);
        let changed = {
            let mut gesture = self.gesture.borrow_mut();
            std::mem::replace(&mut gesture.aim, aim.clone()) != aim
        };
        if changed {
            let shown = aim.as_ref().filter(|folder| **folder != self.root).cloned();
            ctx.call(&self.aimed, (shown,));
        }
        if aim.is_none() {
            return false;
        }
        ctx.accept_drag(Accept::copy_or_move(URI_LIST, offer));
        ctx.set_handled();
        true
    }

    fn keyed(&self, key: Key, modifiers: Modifiers, ctx: &mut EventCtx<'_>) -> bool {
        let plain = modifiers == Modifiers::NONE;
        let action = match key {
            Key::Enter if plain => Some(ExplorerAction::Open),
            Key::F(2) if plain => Some(ExplorerAction::Rename),
            Key::Delete if plain => Some(ExplorerAction::Trash),
            Key::Delete if modifiers.shift && !modifiers.control => Some(ExplorerAction::Erase),
            Key::Char('d') if modifiers == Modifiers::CONTROL => Some(ExplorerAction::Duplicate),
            Key::Char('c') if modifiers == Modifiers::CONTROL | Modifiers::ALT => {
                Some(ExplorerAction::CopyPath)
            }
            Key::Char('c')
                if modifiers == Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT =>
            {
                Some(ExplorerAction::CopyRelativePath)
            }
            Key::ArrowRight | Key::ArrowLeft if plain => {
                ctx.set_handled();
                ctx.call(&self.stepped, (key == Key::ArrowRight,));
                return true;
            }
            Key::Escape if plain => {
                ctx.set_handled();
                ctx.call(&self.cleared, ());
                return true;
            }
            _ => None,
        };
        let Some(action) = action else {
            return false;
        };
        ctx.set_handled();
        ctx.call(&self.acted, (action,));
        true
    }

    fn commanded(&self, command: Command, ctx: &mut EventCtx<'_>) -> bool {
        let action = match command {
            Command::Copy => ExplorerAction::Copy,
            Command::Cut => ExplorerAction::Cut,
            Command::Paste => ExplorerAction::Paste,
            Command::Rename => ExplorerAction::Rename,
            Command::Delete => ExplorerAction::Trash,
            Command::ContextMenu => {
                ctx.set_handled();
                ctx.call(&self.summon, ((None, Point::new(16.0, 16.0)),));
                return true;
            }
            _ => return false,
        };
        ctx.set_handled();
        ctx.call(&self.acted, (action,));
        true
    }
}
