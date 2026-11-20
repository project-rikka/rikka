use super::{Block, Executor, Stage, run_serially};

#[derive(Clone, Copy, Debug, Default)]
pub struct SerialExecutor;

impl Executor for SerialExecutor {
    fn workers(&self) -> usize {
        1
    }

    fn run(&self, stages: &[Stage], task: &(dyn Fn(Block) + Sync)) {
        run_serially(stages, task, 0);
    }
}
