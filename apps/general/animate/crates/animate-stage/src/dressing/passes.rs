use std::sync::Arc;

use animate_shader::SharedCompiler;
use rikka_renderer::Renderer;
use rikka_renderer_3d::{BuiltinEffect, Effect, EffectId, EffectReadiness, Three};
use rikka_shader_compile::pixel_effect;
use rikka_shader_graph::PixelStep;
use rikka_sync::Mutex;
use rustc_hash::FxHashMap;

use crate::dressing::wake::Wake;

#[derive(Debug)]
enum Compiled {
    Pending,
    Done(Result<Effect, String>),
}

#[derive(Debug)]
enum State {
    Compiling(Arc<Mutex<Compiled>>),
    Live {
        id: EffectId,
        readiness: EffectReadiness,
    },
    Failed,
}

#[derive(Debug)]
struct Pass {
    state: State,
    used: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct PassKey {
    source: Arc<str>,
    timed: bool,
}

#[derive(Debug, Default)]
pub(super) struct Passes {
    compiler: SharedCompiler,
    pixels: FxHashMap<PassKey, Pass>,
    builtins: FxHashMap<BuiltinEffect, Option<EffectId>>,
    frame: u64,
}

impl Passes {
    pub(super) fn new(compiler: SharedCompiler) -> Self {
        Self {
            compiler,
            ..Self::default()
        }
    }

    pub(super) fn compiler(&self) -> &SharedCompiler {
        &self.compiler
    }

    pub(super) fn begin(&mut self) {
        self.frame += 1;
    }

    pub(super) fn pixel(
        &mut self,
        renderer: &mut Renderer,
        step: &PixelStep,
        wake: &Wake,
        failures: &mut Vec<String>,
    ) -> Option<EffectId> {
        let key = PassKey {
            source: Arc::from(step.source.as_str()),
            timed: step.timed,
        };
        let frame = self.frame;
        let pass = self.pixels.entry(key).or_insert_with(|| Pass {
            state: State::Compiling(compile(&self.compiler, step, wake)),
            used: frame,
        });
        pass.used = frame;
        if let State::Compiling(slot) = &pass.state {
            let done = match &mut *slot.lock() {
                Compiled::Pending => None,
                compiled @ Compiled::Done(_) => {
                    match std::mem::replace(compiled, Compiled::Pending) {
                        Compiled::Done(result) => Some(result),
                        Compiled::Pending => None,
                    }
                }
            };
            match done {
                None => {
                    wake.wait();
                    return None;
                }
                Some(Err(reason)) => {
                    failures.push(format!("a composite pass failed to compile: {reason}"));
                    pass.state = State::Failed;
                }
                Some(Ok(effect)) => {
                    let created = renderer.three().create_effect(&effect).and_then(|id| {
                        let readiness = renderer.three().effect_readiness(id)?;
                        Ok((id, readiness))
                    });
                    pass.state = match created {
                        Ok((id, readiness)) => {
                            readiness.on_ready(wake.rouse());
                            State::Live { id, readiness }
                        }
                        Err(error) => {
                            failures.push(format!("cannot create a composite pass: {error}"));
                            State::Failed
                        }
                    };
                }
            }
        }
        match &pass.state {
            State::Live { id, readiness } => match readiness.is_ready() {
                true => Some(*id),
                false => {
                    wake.wait();
                    None
                }
            },
            State::Compiling(_) | State::Failed => None,
        }
    }

    pub(super) fn builtin(
        &mut self,
        renderer: &mut Renderer,
        effect: BuiltinEffect,
        failures: &mut Vec<String>,
    ) -> Option<EffectId> {
        *self.builtins.entry(effect).or_insert_with(|| {
            match renderer.three().create_effect(&effect.effect()) {
                Ok(id) => Some(id),
                Err(error) => {
                    failures.push(format!("cannot create {}: {error}", effect.name()));
                    None
                }
            }
        })
    }

    pub(super) fn sweep(&mut self, renderer: &mut Renderer) {
        let frame = self.frame;
        let unused: Vec<PassKey> = self
            .pixels
            .iter()
            .filter(|(_, pass)| pass.used != frame)
            .map(|(key, _)| key.clone())
            .collect();
        for key in unused {
            let Some(pass) = self.pixels.remove(&key) else {
                continue;
            };
            if let State::Live { id, .. } = pass.state
                && let Err(error) = renderer.three().destroy_effect(id)
            {
                tracing::warn!(%error, "failed to destroy a composite pass");
            }
        }
    }
}

fn compile(compiler: &SharedCompiler, step: &PixelStep, wake: &Wake) -> Arc<Mutex<Compiled>> {
    let slot = Arc::new(Mutex::new(Compiled::Pending));
    let filled = Arc::clone(&slot);
    let compiler = compiler.clone();
    let step = step.clone();
    let rouse = wake.rouse();
    let spawned = std::thread::Builder::new()
        .name("rikka-animate-pass".into())
        .spawn(move || {
            let result = compiler
                .get()
                .map_err(|error| error.to_string())
                .and_then(|compiler| {
                    pixel_effect(&step, &compiler, "animate.composite.pass")
                        .map_err(|error| error.to_string())
                });
            *filled.lock() = Compiled::Done(result);
            rouse();
        });
    if let Err(error) = spawned {
        *slot.lock() = Compiled::Done(Err(error.to_string()));
    }
    slot
}
