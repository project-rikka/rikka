use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub struct Opacity(Rc<RefCell<Levels>>);

#[derive(Debug, Default)]
struct Levels {
    drawn: HashMap<String, f32>,
    shown: HashMap<String, f32>,
}

impl Opacity {
    pub fn set(&self, output: &str, opacity: f32) {
        place(&mut self.0.borrow_mut().drawn, output, opacity);
    }
}

pub trait OpacityLevels {
    fn drawn(&self, output: &str) -> f32;

    fn show(&self, output: &str, opacity: f32);

    fn shown(&self, output: &str) -> f32;

    fn clear(&self, output: &str);
}

impl OpacityLevels for Opacity {
    fn drawn(&self, output: &str) -> f32 {
        level(&self.0.borrow().drawn, output)
    }

    fn show(&self, output: &str, opacity: f32) {
        place(&mut self.0.borrow_mut().shown, output, opacity);
    }

    fn shown(&self, output: &str) -> f32 {
        level(&self.0.borrow().shown, output)
    }

    fn clear(&self, output: &str) {
        let mut levels = self.0.borrow_mut();
        levels.drawn.remove(output);
        levels.shown.remove(output);
    }
}

fn place(levels: &mut HashMap<String, f32>, output: &str, opacity: f32) {
    let opacity = opacity.clamp(0.0, 1.0);
    match levels.get_mut(output) {
        Some(held) => *held = opacity,
        None => {
            levels.insert(output.to_owned(), opacity);
        }
    }
}

fn level(levels: &HashMap<String, f32>, output: &str) -> f32 {
    levels.get(output).copied().unwrap_or(0.0)
}
