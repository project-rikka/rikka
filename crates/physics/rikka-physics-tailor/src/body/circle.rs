use glam::Vec2;

const SLACK: f32 = 1e-6;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Circle {
    pub(crate) centre: Vec2,
    pub(crate) radius: f32,
}

impl Circle {
    fn holds(&self, point: Vec2) -> bool {
        point.distance(self.centre) <= self.radius + SLACK * (1.0 + self.radius)
    }

    fn from_diameter(a: Vec2, b: Vec2) -> Self {
        let centre = (a + b) * 0.5;
        Self {
            centre,
            radius: a.distance(centre),
        }
    }

    fn circumscribed(a: Vec2, b: Vec2, c: Vec2) -> Self {
        let (ab, ac) = (b - a, c - a);
        let twice_area = 2.0 * ab.perp_dot(ac);
        if twice_area.abs() <= f32::EPSILON * ab.length_squared().max(ac.length_squared()) {
            let widest = [(a, b), (a, c), (b, c)]
                .into_iter()
                .max_by(|x, y| x.0.distance(x.1).total_cmp(&y.0.distance(y.1)))
                .unwrap_or((a, b));
            return Self::from_diameter(widest.0, widest.1);
        }
        let offset = Vec2::new(
            ac.y * ab.length_squared() - ab.y * ac.length_squared(),
            ab.x * ac.length_squared() - ac.x * ab.length_squared(),
        ) / twice_area;
        Self {
            centre: a + offset,
            radius: offset.length(),
        }
    }
}

pub(crate) fn enclosing(points: &[Vec2]) -> Option<Circle> {
    let mut order: Vec<Vec2> = points.to_vec();
    shuffle(&mut order);
    let (&first, rest) = order.split_first()?;
    let mut circle = Circle {
        centre: first,
        radius: 0.0,
    };
    for (i, &p) in rest.iter().enumerate() {
        if circle.holds(p) {
            continue;
        }
        circle = Circle {
            centre: p,
            radius: 0.0,
        };
        for (j, &q) in order[..=i].iter().enumerate() {
            if circle.holds(q) {
                continue;
            }
            circle = Circle::from_diameter(p, q);
            for &r in &order[..j] {
                if !circle.holds(r) {
                    circle = Circle::circumscribed(p, q, r);
                }
            }
        }
    }
    Some(circle)
}

fn shuffle(points: &mut [Vec2]) {
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    for index in (1..points.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        points.swap(index, (state % (index as u64 + 1)) as usize);
    }
}
