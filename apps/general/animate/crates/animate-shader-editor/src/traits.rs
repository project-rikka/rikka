use animate_look::{Look, LookEdit, Name, Target};
use animate_shader::{Merge, SharedShaders, Workbench};
use rikka_renderer_3d::TextureKind;
use rikka_shader_graph::SocketValue;

use crate::state::ShaderEditor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyState {
    Unkeyed,
    Animated,
    Keyed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetFilter {
    Texture(TextureKind),
    Files(&'static [&'static str]),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotMaterial {
    pub index: u32,
    pub name: String,
    pub class: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotFigure {
    pub name: Name,
    pub materials: Vec<SlotMaterial>,
}

pub trait ShaderEditing {
    fn shader(&self) -> &ShaderEditor;
    fn shader_mut(&mut self) -> &mut ShaderEditor;
    fn workbench(&self) -> &Workbench;
    fn look(&self) -> &Look;
    fn edit_look(
        &mut self,
        edit: LookEdit,
        merge: Option<Merge>,
    ) -> Result<(), animate_look::Error>;
    fn seal_look(&mut self);
    fn undo(&mut self);
    fn redo(&mut self);
    fn can_undo(&self) -> bool;
    fn can_redo(&self) -> bool;
    fn reveal_properties(&mut self);
}

pub trait MaterialGraphs {
    fn slot_figure(&self) -> Option<SlotFigure>;
    fn highlight_slots(&mut self, figure: Name, materials: &[u32]);
}

pub trait ShaderCompiling {
    fn shaders(&self) -> &SharedShaders;
}

pub trait LookValues {
    fn value(&self, target: &Target) -> Option<SocketValue>;
    fn set_value(&mut self, target: Target, value: Option<SocketValue>, merge: Option<Merge>);
    fn key_state(&self, target: &Target) -> KeyState;
    fn toggle_key(&mut self, target: Target);
    fn choose_asset(&mut self, target: Target, filter: AssetFilter);
}

pub trait Workshop: ShaderEditing + MaterialGraphs + ShaderCompiling + LookValues {}

impl<T: ShaderEditing + MaterialGraphs + ShaderCompiling + LookValues> Workshop for T {}

pub type Desk<'a> = animate_ui::Desk<'a, dyn Workshop>;

pub type Reach = animate_ui::Reach<dyn Workshop>;
