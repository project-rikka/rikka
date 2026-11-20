use rikka_image_edit::{Mark, RemovedMark, Sketch, Tool};
use rikka_types::{Color, Point};

fn stroke(sketch: &mut Sketch, from: f32) {
    let mut mark = Mark::new(Tool::Pen, Color::WHITE, 4.0, Point::new(from, 0.0));
    mark.extend_to(Point::new(from, 100.0));
    sketch.push(mark);
}

fn rub(sketch: &mut Sketch, at: Point) -> Vec<RemovedMark> {
    sketch.remove_at(at, 6.0).into_iter().collect()
}

#[test]
fn undo_redo_until_new_mark() {
    let mut sketch = Sketch::new();
    let first = sketch.revision();
    stroke(&mut sketch, 10.0);
    stroke(&mut sketch, 20.0);
    assert_ne!(sketch.revision(), first);

    assert!(sketch.undo());
    assert_eq!(sketch.marks().len(), 1);
    assert!(sketch.redo());
    assert_eq!(sketch.marks().len(), 2);
    assert!(!sketch.redo(), "redo stack empty");

    assert!(sketch.undo());
    stroke(&mut sketch, 30.0);
    assert!(!sketch.can_redo());
    assert_eq!(sketch.marks().len(), 2);
}

#[test]
fn eraser_sweep_removes_topmost_marks_as_one_undo() {
    let mut sketch = Sketch::new();
    stroke(&mut sketch, 10.0);
    stroke(&mut sketch, 10.5);
    stroke(&mut sketch, 30.0);

    let mut removed = rub(&mut sketch, Point::new(10.0, 50.0));
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].index, 1, "topmost mark removed");
    removed.extend(rub(&mut sketch, Point::new(30.0, 50.0)));
    removed.extend(rub(&mut sketch, Point::new(80.0, 50.0)));
    sketch.commit_erase(removed);
    assert_eq!(sketch.marks().len(), 1);

    assert!(sketch.undo());
    assert_eq!(sketch.marks().len(), 3, "both came back together");
    assert!(sketch.redo());
    assert_eq!(sketch.marks().len(), 1);
}
