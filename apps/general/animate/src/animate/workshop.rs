use animate_cast::Subject;
use animate_edit::KeyKind;
use animate_inspector::Pane;
use animate_look::{Look, LookEdit, Name, Target};
use animate_shader::{Merge, SharedShaders, Workbench};
use animate_shader_editor::{
    AssetFilter, KeyState, LookValues, MaterialGraphs, ShaderCompiling, ShaderEditing,
    ShaderEditor, SlotFigure, SlotMaterial,
};
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_model::Model;
use rikka_renderer_3d::TextureKind;
use rikka_shader_graph::SocketValue;

use crate::animate::Animate;

const LOOKUPS: [&str; 2] = ["*.cube", "*.3dl"];

impl Animate {
    fn frame_curve(&self) -> (u32, rikka_model::mmd::VmdCurve) {
        (self.transport.frame(), self.curves.value(KeyKind::Look))
    }

    fn look_failed(&mut self, error: animate_look::Error) {
        self.notice = Some(Label::from(error.to_string()));
    }

    fn chosen_asset(&mut self, target: Target, path: std::path::PathBuf) {
        let value = SocketValue::Asset(path.to_string_lossy().into_owned());
        LookValues::set_value(self, target, Some(value), None);
    }
}

impl ShaderEditing for Animate {
    fn shader(&self) -> &ShaderEditor {
        &self.shader
    }

    fn shader_mut(&mut self) -> &mut ShaderEditor {
        &mut self.shader
    }

    fn workbench(&self) -> &Workbench {
        &self.workbench
    }

    fn look(&self) -> &Look {
        Animate::look(self)
    }

    fn edit_look(
        &mut self,
        edit: LookEdit,
        merge: Option<Merge>,
    ) -> Result<(), animate_look::Error> {
        Animate::edit_look(self, edit, merge)
    }

    fn seal_look(&mut self) {
        self.history.seal();
    }

    fn undo(&mut self) {
        Animate::undo(self);
    }

    fn redo(&mut self) {
        Animate::redo(self);
    }

    fn can_undo(&self) -> bool {
        Animate::can_undo(self)
    }

    fn can_redo(&self) -> bool {
        Animate::can_redo(self)
    }

    fn reveal_properties(&mut self) {
        self.inspecting = true;
        self.controls.pane = Pane::Node;
    }
}

impl MaterialGraphs for Animate {
    fn slot_figure(&self) -> Option<SlotFigure> {
        match self.subject? {
            Subject::Figure(id) => {
                let figure = self.cast.figure(id)?;
                Some(slot_figure(&figure.name, &figure.model, |index| {
                    figure.material_class(index)
                }))
            }
            subject @ (Subject::Stage | Subject::Accessory(_)) => {
                let prop = &self.cast.prop(subject)?.prop;
                Some(slot_figure(&prop.name, &prop.model, |index| {
                    prop.material_class(index)
                }))
            }
            Subject::Emitter(_) | Subject::Camera | Subject::Look | Subject::Wind => None,
        }
    }

    fn highlight_slots(&mut self, figure: Name, materials: &[u32]) {
        if let Some(id) = self
            .cast
            .figures()
            .iter()
            .find(|held| figure == held.name.as_str())
            .map(|held| held.id)
        {
            self.stage.lock().highlight(id, materials);
            return;
        }
        if let Some(id) = self
            .cast
            .props()
            .find(|track| figure == track.prop.name.as_str())
            .map(|track| track.prop.id)
        {
            self.stage.lock().highlight_prop(id, materials);
        }
    }
}

fn slot_figure(
    name: &str,
    model: &Model,
    class: impl Fn(u32) -> Option<&'static str>,
) -> SlotFigure {
    SlotFigure {
        name: Name::new(name),
        materials: model
            .materials
            .iter()
            .enumerate()
            .map(|(index, material)| SlotMaterial {
                index: index as u32,
                name: material.name.clone(),
                class: class(index as u32),
            })
            .collect(),
    }
}

impl ShaderCompiling for Animate {
    fn shaders(&self) -> &SharedShaders {
        &self.shaders
    }
}

impl LookValues for Animate {
    fn value(&self, target: &Target) -> Option<SocketValue> {
        let look = Animate::look(self);
        let frame = self.transport.frame() as f32;
        let value = look.value_at(self.workbench.library(), target, frame)?;
        look.value_to_socket(target, &value)
    }

    fn set_value(&mut self, target: Target, value: Option<SocketValue>, merge: Option<Merge>) {
        let (frame, curve) = self.frame_curve();
        let changed = self.change_look(merge, |look, library| {
            let value = match value {
                Some(value) => Some(
                    look.value_from_socket(library, &target, &value)
                        .ok_or_else(|| animate_look::Error::Key {
                            target: target.to_string(),
                            reason: "the value does not fit".into(),
                        })?,
                ),
                None => None,
            };
            look.set_value(library, &target, value, frame, curve)
                .map(drop)
        });
        if let Err(error) = changed {
            self.look_failed(error);
        }
    }

    fn key_state(&self, target: &Target) -> KeyState {
        let look = Animate::look(self);
        let keys = look.keys.tracks().keys(target);
        let frame = self.transport.frame();
        match (keys.is_empty(), keys.iter().any(|key| key.frame == frame)) {
            (true, _) => KeyState::Unkeyed,
            (false, true) => KeyState::Keyed,
            (false, false) => KeyState::Animated,
        }
    }

    fn toggle_key(&mut self, target: Target) {
        let (frame, curve) = self.frame_curve();
        let changed = self.change_look(None, |look, library| {
            let value = look
                .value_at(library, &target, frame as f32)
                .or_else(|| look.default_value(library, &target))
                .ok_or_else(|| animate_look::Error::Key {
                    target: target.to_string(),
                    reason: "the input has no value to key".into(),
                })?;
            look.toggle_key(library, &target, value, frame, curve)
                .map(drop)
        });
        if let Err(error) = changed {
            self.look_failed(error);
        }
    }

    fn choose_asset(&mut self, target: Target, filter: AssetFilter) {
        let files = match filter {
            AssetFilter::Texture(TextureKind::Colour) => LOOKUPS.into_iter().fold(
                Filter::named(tr!("animate.look.images").to_string()).mime_type("image/*"),
                Filter::pattern,
            ),
            AssetFilter::Texture(_) => {
                Filter::named(tr!("animate.look.images").to_string()).mime_type("image/*")
            }
            AssetFilter::Files(extensions) => extensions.iter().fold(
                Filter::named(tr!("animate.look.files").to_string()),
                |filter, extension| filter.pattern(format!("*.{extension}")),
            ),
        };
        let chooser = Chooser::open(tr!("animate.look.pick_file").to_string())
            .parent(window_identifier())
            .accept_label(tr!("animate.add.accept").to_string())
            .filter(files);
        let chooser = match &self.folder {
            Some(folder) => chooser.current_folder(folder),
            None => chooser,
        };
        spawn_blocking(
            move || chooser.run(),
            move |animate: &mut Animate, chosen: rikka_dialog::Result<ChooserResponse>| match chosen
            {
                Ok(ChooserResponse::Files(paths)) => match paths.into_iter().next() {
                    Some(path) => animate.chosen_asset(target.clone(), path),
                    None => skip_view(),
                },
                Ok(ChooserResponse::Cancelled) => skip_view(),
                Err(error) => animate.notice = Some(Label::from(animate_ui::error_chain(&error))),
            },
        );
    }
}
