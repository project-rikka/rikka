use std::sync::{Arc, Weak};

use ash::vk;
use rikka_sync::Mutex;
use rustc_hash::FxHashMap;

use crate::fx::effect::{BackdropShader, WeakShader};
use crate::fx::pool::FX_FORMAT;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::pipeline::{
    Blend, Compiling, GraphicsPipelineDesc, Pipeline, PipelineHandles,
};
use rikka_gpu::error::Result;

static WATCHING: Mutex<Vec<Weak<FxPipelines>>> = Mutex::new(Vec::new());

static CUSTOM: Mutex<Vec<WeakShader>> = Mutex::new(Vec::new());

pub(crate) fn precompile_all(shader: &BackdropShader) {
    {
        let mut custom = CUSTOM.lock();
        custom.retain(WeakShader::alive);
        custom.push(shader.downgrade());
    }
    let watching: Vec<Arc<FxPipelines>> =
        WATCHING.lock().iter().filter_map(Weak::upgrade).collect();
    for pipelines in watching {
        pipelines.precompile(shader, FX_FORMAT, Blend::Opaque);
    }
}

type Key = (u64, vk::Format, Blend);

struct PipelineSet {
    open: bool,
    pipelines: FxHashMap<Key, Compiling<Pipeline>>,
}

pub(crate) struct FxPipelines {
    device: Arc<RenderDevice>,
    table_layout: vk::DescriptorSetLayout,
    push_size: u32,
    built: Mutex<PipelineSet>,
}

impl FxPipelines {
    pub(crate) fn start(
        device: Arc<RenderDevice>,
        table_layout: vk::DescriptorSetLayout,
        push_size: u32,
        family: impl IntoIterator<Item = (BackdropShader, vk::Format, Blend)>,
    ) -> Arc<Self> {
        let pipelines = Arc::new(Self {
            device,
            table_layout,
            push_size,
            built: Mutex::new(PipelineSet {
                open: true,
                pipelines: FxHashMap::default(),
            }),
        });
        let mut watching = WATCHING.lock();
        watching.retain(|watcher| watcher.strong_count() > 0);
        watching.push(Arc::downgrade(&pipelines));
        drop(watching);
        for (shader, format, blend) in family {
            pipelines.precompile(&shader, format, blend);
        }
        let custom: Vec<BackdropShader> = CUSTOM
            .lock()
            .iter()
            .filter_map(WeakShader::upgrade)
            .collect();
        for shader in custom {
            pipelines.precompile(&shader, FX_FORMAT, Blend::Opaque);
        }
        pipelines
    }

    pub(crate) fn get(
        &self,
        shader: &BackdropShader,
        format: vk::Format,
        blend: Blend,
    ) -> Result<PipelineHandles> {
        let mut built = self.built.lock();
        let compiling = built
            .pipelines
            .entry((shader.hash(), format, blend))
            .or_insert_with(|| self.compile(shader, format, blend));
        Ok(compiling.get()?.handles())
    }

    pub(crate) fn close(&self) {
        let mut built = self.built.lock();
        built.open = false;
        built.pipelines.clear();
    }

    fn precompile(&self, shader: &BackdropShader, format: vk::Format, blend: Blend) {
        let mut built = self.built.lock();
        if !built.open {
            return;
        }
        built
            .pipelines
            .entry((shader.hash(), format, blend))
            .or_insert_with(|| self.compile(shader, format, blend));
    }

    fn compile(
        &self,
        shader: &BackdropShader,
        format: vk::Format,
        blend: Blend,
    ) -> Compiling<Pipeline> {
        let shader = shader.clone();
        let (table_layout, push_size) = (self.table_layout, self.push_size);
        Compiling::start(&self.device, move |device| {
            Pipeline::build(
                device,
                shader.label(),
                &GraphicsPipelineDesc {
                    vertex_spv: shaders_gpu::FULLSCREEN_VERT.spirv,
                    fragment_spv: shader.spirv(),
                    set_layouts: &[table_layout],
                    push_constant_size: push_size,
                    push_constant_stages: vk::ShaderStageFlags::FRAGMENT,
                    color_format: format,
                    blend,
                },
            )
        })
    }
}
