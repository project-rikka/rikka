use crate::widget::Widget;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WidgetMatch {
    pub matched: Vec<(usize, usize)>,
    pub removed: Vec<usize>,
    pub added: Vec<usize>,
}

#[must_use]
pub fn pair(before: &[Widget], after: &[Widget]) -> WidgetMatch {
    let mut taken_before = vec![false; before.len()];
    let mut taken_after = vec![false; after.len()];
    let mut matched = Vec::new();

    for (to, widget) in after.iter().enumerate() {
        let same = before.iter().enumerate().position(|(from, earlier)| {
            !taken_before[from] && earlier.id == widget.id && earlier.kind() == widget.kind()
        });
        if let Some(from) = same {
            taken_before[from] = true;
            taken_after[to] = true;
            matched.push((from, to));
        }
    }

    let mut alike: Vec<(f32, usize, usize)> = before
        .iter()
        .enumerate()
        .filter(|(from, _)| !taken_before[*from])
        .flat_map(|(from, earlier)| {
            after
                .iter()
                .enumerate()
                .filter(|(to, later)| !taken_after[*to] && later.kind() == earlier.kind())
                .map(move |(to, later)| (apart(earlier, later), from, to))
        })
        .collect();
    alike.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    for (_, from, to) in alike {
        if !taken_before[from] && !taken_after[to] {
            taken_before[from] = true;
            taken_after[to] = true;
            matched.push((from, to));
        }
    }

    matched.sort_by_key(|(_, to)| *to);
    WidgetMatch {
        matched,
        removed: unpaired(&taken_before),
        added: unpaired(&taken_after),
    }
}

fn apart(a: &Widget, b: &Widget) -> f32 {
    let dx = a.spot.x - b.spot.x;
    let dy = a.spot.y - b.spot.y;
    dx * dx + dy * dy
}

fn unpaired(taken: &[bool]) -> Vec<usize> {
    taken
        .iter()
        .enumerate()
        .filter(|(_, taken)| !**taken)
        .map(|(at, _)| at)
        .collect()
}
