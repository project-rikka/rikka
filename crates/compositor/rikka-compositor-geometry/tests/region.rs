use rikka_compositor_geometry::{Logical, Point, Rect, Region};

fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect<i32, Logical> {
    Rect::new((x, y).into(), (w, h).into())
}

fn area(region: &Region<Logical>) -> i32 {
    region.iter().map(|r| r.size.w * r.size.h).sum()
}

fn assert_disjoint(region: &Region<Logical>) {
    let rects: Vec<_> = region.iter().collect();
    for (index, a) in rects.iter().enumerate() {
        for b in &rects[index + 1..] {
            assert!(!a.overlaps(*b), "{a:?} overlaps {b:?}");
        }
    }
}

const LOW: i32 = -3;
const HIGH: i32 = 13;

struct Noise(u64);

impl Noise {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: i32) -> i32 {
        (self.next() % bound as u64) as i32
    }

    fn rect(&mut self) -> Rect<i32, Logical> {
        let x = LOW + self.below(HIGH - LOW);
        let y = LOW + self.below(HIGH - LOW);
        let w = self.below(HIGH - x + 1).min(9);
        let h = self.below(HIGH - y + 1).min(9);
        rect(x, y, w, h)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Pixels(Vec<bool>);

impl Pixels {
    fn empty() -> Self {
        Pixels(vec![false; ((HIGH - LOW) * (HIGH - LOW)) as usize])
    }

    fn at(&self, x: i32, y: i32) -> bool {
        self.0[((y - LOW) * (HIGH - LOW) + (x - LOW)) as usize]
    }

    fn paint(&mut self, area: Rect<i32, Logical>, value: bool) {
        for (x, y) in cells(area) {
            self.0[((y - LOW) * (HIGH - LOW) + (x - LOW)) as usize] = value;
        }
    }

    fn keep_only(&mut self, area: Rect<i32, Logical>) {
        for (x, y) in cells(rect(LOW, LOW, HIGH - LOW, HIGH - LOW)) {
            if !area.contains((x, y)) {
                self.0[((y - LOW) * (HIGH - LOW) + (x - LOW)) as usize] = false;
            }
        }
    }

    fn union(&mut self, other: &Pixels) {
        for (mine, theirs) in self.0.iter_mut().zip(&other.0) {
            *mine |= *theirs;
        }
    }

    fn count(&self) -> i32 {
        self.0.iter().filter(|cell| **cell).count() as i32
    }

    fn row_runs(&self) -> Region<Logical> {
        let mut region = Region::new();
        for y in LOW..HIGH {
            let mut x = LOW;
            while x < HIGH {
                if !self.at(x, y) {
                    x += 1;
                    continue;
                }
                let start = x;
                while x < HIGH && self.at(x, y) {
                    x += 1;
                }
                region.add(rect(start, y, x - start, 1));
            }
        }
        region
    }
}

fn cells(area: Rect<i32, Logical>) -> impl Iterator<Item = (i32, i32)> {
    (area.loc.y..area.loc.y + area.size.h)
        .flat_map(move |y| (area.loc.x..area.loc.x + area.size.w).map(move |x| (x, y)))
}

fn agrees(region: &Region<Logical>, pixels: &Pixels, noise: &mut Noise) {
    for (x, y) in cells(rect(LOW, LOW, HIGH - LOW, HIGH - LOW)) {
        assert_eq!(region.contains((x, y)), pixels.at(x, y), "pixel {x},{y}");
    }
    assert_disjoint(region);
    assert_eq!(area(region), pixels.count());
    for piece in region.iter() {
        assert!(cells(piece).all(|(x, y)| pixels.at(x, y)), "{piece:?}");
    }
    assert_eq!(*region, pixels.row_runs(), "the band form is canonical");
    let bounds = region.bounding_box();
    if pixels.count() == 0 {
        assert_eq!(bounds, Rect::zero());
    } else {
        let set: Vec<(i32, i32)> = cells(rect(LOW, LOW, HIGH - LOW, HIGH - LOW))
            .filter(|(x, y)| pixels.at(*x, *y))
            .collect();
        let left = set.iter().map(|(x, _)| *x).min().unwrap_or_default();
        let right = set.iter().map(|(x, _)| *x).max().unwrap_or_default() + 1;
        let top = set.iter().map(|(_, y)| *y).min().unwrap_or_default();
        let bottom = set.iter().map(|(_, y)| *y).max().unwrap_or_default() + 1;
        assert_eq!(bounds, rect(left, top, right - left, bottom - top));
    }
    for _ in 0..8 {
        let probe = noise.rect();
        let covered = cells(probe).all(|(x, y)| pixels.at(x, y));
        assert_eq!(region.covers(probe), covered, "covers {probe:?}");
    }
}

#[test]
fn operations_match_pixel_model() {
    let mut noise = Noise(0x9e37_79b9_7f4a_7c15);
    for _ in 0..300 {
        let mut region = Region::new();
        let mut pixels = Pixels::empty();
        for _ in 0..12 {
            match noise.below(5) {
                0 | 1 => {
                    let added = noise.rect();
                    region.add(added);
                    pixels.paint(added, true);
                }
                2 => {
                    let punched = noise.rect();
                    region.subtract(punched);
                    pixels.paint(punched, false);
                }
                3 => {
                    let mut other = Region::new();
                    let mut painted = Pixels::empty();
                    for _ in 0..1 + noise.below(4) {
                        let added = noise.rect();
                        other.add(added);
                        painted.paint(added, true);
                    }
                    region.union(&other);
                    pixels.union(&painted);
                }
                _ => {
                    let area = noise.rect();
                    region = region.intersect(area);
                    pixels.keep_only(area);
                }
            }
            agrees(&region, &pixels, &mut noise);
        }
    }
}

#[test]
fn translating_moves_every_pixel() {
    let mut noise = Noise(0x2545_f491_4f6c_dd1d);
    for _ in 0..100 {
        let rects: Vec<_> = (0..6).map(|_| noise.rect()).collect();
        let mut region: Region<Logical> = rects.iter().copied().collect();
        let (dx, dy) = (noise.below(7) - 3, noise.below(7) - 3);
        region.translate(Point::from((dx, dy)));
        for (x, y) in cells(rect(LOW - 4, LOW - 4, HIGH - LOW + 8, HIGH - LOW + 8)) {
            let inside = rects.iter().any(|r| r.contains((x - dx, y - dy)));
            assert_eq!(region.contains((x, y)), inside);
        }
    }
}

#[test]
fn fine_checkerboard_stays_exact() {
    let mut region = Region::<Logical>::new();
    for y in 0..64 {
        for x in 0..64 {
            if (x + y) % 2 == 0 {
                region.add(rect(x, y, 1, 1));
            }
        }
    }
    assert_eq!(region.iter().len(), 64 * 32);
    for y in 0..64 {
        region.subtract(rect(0, y, 64, 1));
        region.add(rect(0, y, 64, 1));
    }
    assert_eq!(region.iter().collect::<Vec<_>>(), [rect(0, 0, 64, 64)]);
    assert!(region.covers(rect(3, 3, 60, 60)));
    assert!(!region.covers(rect(3, 3, 62, 62)));
}
