use super::homography::Homography;
use crate::camera::Camera;
use crate::placement::Placement;
use crate::project::Format;

type Vector = [f64; 3];
type Matrix = [[f64; 3]; 3];

#[derive(Clone, Copy, Debug)]
pub(crate) struct Plane {
    origin: Vector,
    across: Vector,
    down: Vector,
    anchor: Vector,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Lens {
    turn: Matrix,
    eye: Vector,
    focal: f64,
    center: [f64; 2],
}

impl Plane {
    pub(crate) fn from_placement(placement: &Placement) -> Plane {
        let degrees = |value: f32| f64::from(value).to_radians();
        let turn = match placement.three_d {
            true => product(
                product(
                    about_x(degrees(placement.tilt_x)),
                    about_y(degrees(placement.tilt_y)),
                ),
                about_z(degrees(placement.rotation)),
            ),
            false => about_z(degrees(placement.rotation)),
        };
        let (across, down) = placement.scale();
        let (across, down) = (f64::from(across), f64::from(down));
        let anchor = [
            f64::from(placement.x),
            f64::from(placement.y),
            match placement.three_d {
                true => f64::from(placement.depth),
                false => 0.0,
            },
        ];
        let shift = apply(
            turn,
            [
                across * f64::from(placement.anchor.x),
                down * f64::from(placement.anchor.y),
                0.0,
            ],
        );
        Plane {
            origin: sub(anchor, shift),
            across: apply(turn, [across, 0.0, 0.0]),
            down: apply(turn, [0.0, down, 0.0]),
            anchor,
        }
    }

    pub(crate) fn level(placement: &Placement) -> Plane {
        let depth = match placement.three_d {
            true => f64::from(placement.depth),
            false => 0.0,
        };
        Plane {
            origin: [0.0, 0.0, depth],
            across: [1.0, 0.0, 0.0],
            down: [0.0, 1.0, 0.0],
            anchor: [0.0, 0.0, depth],
        }
    }

    pub(crate) fn picture(format: Format) -> Plane {
        let (width, height) = (f64::from(format.width), f64::from(format.height));
        Plane {
            origin: [-width / 2.0, -height / 2.0, 0.0],
            across: [1.0, 0.0, 0.0],
            down: [0.0, 1.0, 0.0],
            anchor: [0.0, 0.0, 0.0],
        }
    }
}

impl Lens {
    pub(crate) fn new(camera: &Camera, format: Format) -> Lens {
        let degrees = |value: f32| f64::from(value).to_radians();
        let width = f64::from(format.width);
        let focal = camera.focal(format);
        let distance = focal - f64::from(camera.dolly);
        let turn = product(
            product(about_y(degrees(camera.pan)), about_x(degrees(camera.tilt))),
            about_z(degrees(camera.roll)),
        );
        let forward = apply(turn, [0.0, 0.0, 1.0]);
        let interest = [
            f64::from(camera.interest.x),
            f64::from(camera.interest.y),
            0.0,
        ];
        Lens {
            turn,
            eye: sub(interest, forward.map(|axis| axis * distance)),
            focal: focal * f64::from(camera.zoom),
            center: [width / 2.0, f64::from(format.height) / 2.0],
        }
    }

    pub(crate) fn project(&self, plane: &Plane) -> Homography {
        let back = transpose(self.turn);
        let columns = [
            apply(back, plane.across),
            apply(back, plane.down),
            apply(back, sub(plane.origin, self.eye)),
        ];
        let [x, y] = self.center;
        Homography::new(std::array::from_fn(|row| {
            std::array::from_fn(|column| {
                let axis = columns[column];
                match row {
                    0 => self.focal * axis[0] + x * axis[2],
                    1 => self.focal * axis[1] + y * axis[2],
                    _ => axis[2],
                }
            })
        }))
    }

    pub(crate) fn depth(&self, plane: &Plane) -> f64 {
        let eye_space = apply(transpose(self.turn), sub(plane.anchor, self.eye));
        eye_space[2]
    }
}

fn about_x(angle: f64) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    [[1.0, 0.0, 0.0], [0.0, cos, -sin], [0.0, sin, cos]]
}

fn about_y(angle: f64) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    [[cos, 0.0, sin], [0.0, 1.0, 0.0], [-sin, 0.0, cos]]
}

fn about_z(angle: f64) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    [[cos, -sin, 0.0], [sin, cos, 0.0], [0.0, 0.0, 1.0]]
}

fn product(left: Matrix, right: Matrix) -> Matrix {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            (0..3)
                .map(|inner| left[row][inner] * right[inner][column])
                .sum()
        })
    })
}

fn transpose(matrix: Matrix) -> Matrix {
    std::array::from_fn(|row| std::array::from_fn(|column| matrix[column][row]))
}

fn apply(matrix: Matrix, vector: Vector) -> Vector {
    matrix.map(|row| row[0] * vector[0] + row[1] * vector[1] + row[2] * vector[2])
}

fn sub(left: Vector, right: Vector) -> Vector {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}
