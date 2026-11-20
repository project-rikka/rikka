use crate::color::Rgb;

const INDEX_BITS: u32 = 5;
const BITS_TO_REMOVE: u32 = 8 - INDEX_BITS;
const SIDE: usize = (1 << INDEX_BITS) + 1;
const TOTAL: usize = SIDE * SIDE * SIDE;

#[derive(Clone, Copy, Default)]
struct Cube {
    low: [usize; 3],
    high: [usize; 3],
    volume: i64,
}

#[derive(Clone, Copy)]
struct Best {
    cut: Option<usize>,
    maximum: f64,
}

struct Moments {
    weights: Vec<i64>,
    red: Vec<i64>,
    green: Vec<i64>,
    blue: Vec<i64>,
    squares: Vec<f64>,
}

const fn index(red: usize, green: usize, blue: usize) -> usize {
    red * SIDE * SIDE + green * SIDE + blue
}

fn volume(cube: &Cube, moment: &[i64]) -> i64 {
    let (low, high) = (cube.low, cube.high);
    moment[index(high[0], high[1], high[2])]
        - moment[index(high[0], high[1], low[2])]
        - moment[index(high[0], low[1], high[2])]
        + moment[index(high[0], low[1], low[2])]
        - moment[index(low[0], high[1], high[2])]
        + moment[index(low[0], high[1], low[2])]
        + moment[index(low[0], low[1], high[2])]
        - moment[index(low[0], low[1], low[2])]
}

fn corner(cube: &Cube, axis: usize, position: usize, upper: [bool; 2]) -> usize {
    let others = [(axis + 1) % 3, (axis + 2) % 3];
    let mut at = [0; 3];
    at[axis] = position;
    for (other, upper) in others.into_iter().zip(upper) {
        at[other] = if upper {
            cube.high[other]
        } else {
            cube.low[other]
        };
    }
    index(at[0], at[1], at[2])
}

fn bottom(cube: &Cube, axis: usize, moment: &[i64]) -> i64 {
    let at = |upper: [bool; 2]| moment[corner(cube, axis, cube.low[axis], upper)];
    -at([true, true]) + at(ordered(axis, true, false)) + at(ordered(axis, false, true))
        - at([false, false])
}

fn top(cube: &Cube, axis: usize, position: usize, moment: &[i64]) -> i64 {
    let at = |upper: [bool; 2]| moment[corner(cube, axis, position, upper)];
    at([true, true]) - at(ordered(axis, true, false)) - at(ordered(axis, false, true))
        + at([false, false])
}

const fn ordered(axis: usize, first: bool, second: bool) -> [bool; 2] {
    match axis {
        1 => [second, first],
        _ => [first, second],
    }
}

impl Moments {
    fn of(pixels: &[Rgb]) -> Moments {
        let mut moments = Moments {
            weights: vec![0; TOTAL],
            red: vec![0; TOTAL],
            green: vec![0; TOTAL],
            blue: vec![0; TOTAL],
            squares: vec![0.0; TOTAL],
        };
        for pixel in pixels {
            let bin = |component: u8| usize::from(component >> BITS_TO_REMOVE) + 1;
            let at = index(bin(pixel.red), bin(pixel.green), bin(pixel.blue));
            let (red, green, blue) = (
                f64::from(pixel.red),
                f64::from(pixel.green),
                f64::from(pixel.blue),
            );
            moments.weights[at] += 1;
            moments.red[at] += i64::from(pixel.red);
            moments.green[at] += i64::from(pixel.green);
            moments.blue[at] += i64::from(pixel.blue);
            moments.squares[at] += blue.mul_add(blue, red.mul_add(red, green * green));
        }
        moments.accumulate();
        moments
    }

    fn accumulate(&mut self) {
        for red in 1..SIDE {
            let mut area = [0i64; SIDE];
            let mut area_red = [0i64; SIDE];
            let mut area_green = [0i64; SIDE];
            let mut area_blue = [0i64; SIDE];
            let mut area_squares = [0.0f64; SIDE];
            for green in 1..SIDE {
                let mut line = 0;
                let mut line_red = 0;
                let mut line_green = 0;
                let mut line_blue = 0;
                let mut line_squares = 0.0;
                for blue in 1..SIDE {
                    let at = index(red, green, blue);
                    line += self.weights[at];
                    line_red += self.red[at];
                    line_green += self.green[at];
                    line_blue += self.blue[at];
                    line_squares += self.squares[at];
                    area[blue] += line;
                    area_red[blue] += line_red;
                    area_green[blue] += line_green;
                    area_blue[blue] += line_blue;
                    area_squares[blue] += line_squares;
                    let previous = index(red - 1, green, blue);
                    self.weights[at] = self.weights[previous] + area[blue];
                    self.red[at] = self.red[previous] + area_red[blue];
                    self.green[at] = self.green[previous] + area_green[blue];
                    self.blue[at] = self.blue[previous] + area_blue[blue];
                    self.squares[at] = self.squares[previous] + area_squares[blue];
                }
            }
        }
    }

    fn variance(&self, cube: &Cube) -> f64 {
        let red = volume(cube, &self.red) as f64;
        let green = volume(cube, &self.green) as f64;
        let blue = volume(cube, &self.blue) as f64;
        let (low, high) = (cube.low, cube.high);
        let squares = &self.squares;
        let spread = squares[index(high[0], high[1], high[2])]
            - squares[index(high[0], high[1], low[2])]
            - squares[index(high[0], low[1], high[2])]
            + squares[index(high[0], low[1], low[2])]
            - squares[index(low[0], high[1], high[2])]
            + squares[index(low[0], high[1], low[2])]
            + squares[index(low[0], low[1], high[2])]
            - squares[index(low[0], low[1], low[2])];
        let hypotenuse = blue.mul_add(blue, red.mul_add(red, green * green));
        spread - (hypotenuse / volume(cube, &self.weights) as f64)
    }

    fn maximize(&self, cube: &Cube, axis: usize, whole: [i64; 4]) -> Best {
        let planes = [&self.red, &self.green, &self.blue, &self.weights];
        let below = planes.map(|moment| bottom(cube, axis, moment) as f64);
        let mut best = Best {
            cut: None,
            maximum: 0.0,
        };
        for position in cube.low[axis] + 1..cube.high[axis] {
            let mut half: [f64; 4] = std::array::from_fn(|plane| {
                below[plane] + top(cube, axis, position, planes[plane]) as f64
            });
            if half[3] == 0.0 {
                continue;
            }
            let mut score =
                half[2].mul_add(half[2], half[0].mul_add(half[0], half[1].powi(2))) / half[3];
            for (half, whole) in half.iter_mut().zip(whole) {
                *half = whole as f64 - *half;
            }
            if half[3] == 0.0 {
                continue;
            }
            score += half[2].mul_add(half[2], half[0].mul_add(half[0], half[1].powi(2))) / half[3];
            if score > best.maximum {
                best = Best {
                    cut: Some(position),
                    maximum: score,
                };
            }
        }
        best
    }

    fn cut(&self, one: &mut Cube, two: &mut Cube) -> bool {
        let whole = [
            volume(one, &self.red),
            volume(one, &self.green),
            volume(one, &self.blue),
            volume(one, &self.weights),
        ];
        let best: [Best; 3] = std::array::from_fn(|axis| self.maximize(one, axis, whole));
        let [red, green, blue] = best.map(|best| best.maximum);
        let axis = if red >= green && red >= blue {
            0
        } else if green >= red && green >= blue {
            1
        } else {
            2
        };
        let Some(position) = best[axis].cut else {
            return false;
        };
        two.high = one.high;
        two.low = one.low;
        one.high[axis] = position;
        two.low[axis] = position;
        for cube in [one, two] {
            cube.volume = (0..3)
                .map(|axis| cube.high[axis] as i64 - cube.low[axis] as i64)
                .product();
        }
        true
    }
}

pub(crate) fn boxes(pixels: &[Rgb], max_colors: usize) -> Vec<Rgb> {
    if max_colors == 0 {
        return Vec::new();
    }
    let moments = Moments::of(pixels);
    let mut cubes = vec![Cube::default(); max_colors];
    cubes[0].high = [SIDE - 1; 3];
    let mut variances = vec![0.0f64; max_colors];
    let mut next = 0;
    let mut generated = max_colors;
    let mut made = 1;
    while made < max_colors {
        let (mut one, mut two) = (cubes[next], cubes[made]);
        if moments.cut(&mut one, &mut two) {
            cubes[next] = one;
            cubes[made] = two;
            for cube in [next, made] {
                variances[cube] = match cubes[cube].volume > 1 {
                    true => moments.variance(&cubes[cube]),
                    false => 0.0,
                };
            }
        } else {
            variances[next] = 0.0;
            made -= 1;
        }
        next = 0;
        let mut widest = variances[0];
        for (candidate, &variance) in variances.iter().enumerate().take(made + 1).skip(1) {
            if variance > widest {
                widest = variance;
                next = candidate;
            }
        }
        if widest <= 0.0 {
            generated = made + 1;
            break;
        }
        made += 1;
    }
    let mut colors: Vec<Rgb> = Vec::new();
    for cube in &cubes[..generated] {
        let weight = volume(cube, &moments.weights);
        if weight > 0 {
            let mean = |moment: &[i64]| (volume(cube, moment) / weight) as u8;
            let color = Rgb::new(
                mean(&moments.red),
                mean(&moments.green),
                mean(&moments.blue),
            );
            if !colors.contains(&color) {
                colors.push(color);
            }
        }
    }
    colors
}
