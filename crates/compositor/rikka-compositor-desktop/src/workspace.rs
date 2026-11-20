use rikka_compositor_types::OutputId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Workspace {
    output: OutputId,
    number: usize,
}

impl Workspace {
    #[must_use]
    pub fn new(output: OutputId, number: usize) -> Self {
        Self { output, number }
    }

    pub fn output(self) -> OutputId {
        self.output
    }

    pub fn number(self) -> usize {
        self.number
    }
}
