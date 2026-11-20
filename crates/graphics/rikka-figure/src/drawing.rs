use rikka_image_edit::{Marking, Marks, Recipe, Step};

use crate::canvas_transform::CanvasTransform;
use crate::form::Form;
use crate::text::Letterforms;

#[must_use]
pub fn recipe<'a>(forms: impl IntoIterator<Item = &'a Form>, placing: CanvasTransform) -> Recipe {
    let mut recipe = Recipe::new();
    let mut marks: Vec<Marking> = Vec::new();
    let flush = |recipe: &mut Recipe, marks: &mut Vec<Marking>| {
        if !marks.is_empty() {
            recipe.push(Step::new(Marks::new(std::mem::take(marks))));
        }
    };
    for form in forms {
        match form {
            Form::Text(lettering) => {
                flush(&mut recipe, &mut marks);
                let lettered =
                    Letterforms::new(lettering, placing.scale).recipe(&[], placing.offset);
                for step in lettered.steps() {
                    recipe.push(step.clone());
                }
            }
            other => {
                let placed = match placing == CanvasTransform::IDENTITY {
                    true => other.clone(),
                    false => other.mapped(placing.affine(), true),
                };
                marks.extend(placed.marking());
            }
        }
    }
    flush(&mut recipe, &mut marks);
    recipe
}
