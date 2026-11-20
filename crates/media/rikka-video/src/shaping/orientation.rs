use rikka_image::Orientation;

use rikka_ffmpeg::avcodec::{self, AVPacketSideData};
use rikka_ffmpeg::avformat::AVStream;

const MATRIX_BYTES: usize = 9 * size_of::<i32>();

pub(crate) fn from_stream(stream: &AVStream) -> Orientation {
    let parameters = unsafe { &*stream.codecpar };
    let count = usize::try_from(parameters.nb_coded_side_data).unwrap_or(0);
    if parameters.coded_side_data.is_null() || count == 0 {
        return Orientation::Upright;
    }
    let side_data: &[AVPacketSideData] =
        unsafe { std::slice::from_raw_parts(parameters.coded_side_data, count) };
    side_data
        .iter()
        .find(|entry| {
            entry.r#type == avcodec::AV_PKT_DATA_DISPLAYMATRIX
                && entry.size >= MATRIX_BYTES
                && !entry.data.is_null()
        })
        .map_or(Orientation::Upright, |entry| {
            let mut matrix = [0i32; 9];
            for (index, cell) in matrix.iter_mut().enumerate() {
                *cell = unsafe { entry.data.cast::<i32>().add(index).read_unaligned() };
            }
            of_matrix(matrix)
        })
}

fn of_matrix(matrix: [i32; 9]) -> Orientation {
    let Some(turn) = turn(&matrix) else {
        return Orientation::Upright;
    };
    let near = |degrees: f64| (turn - degrees).abs() < 1.0;
    if near(90.0) {
        match matrix[3] > 0 {
            true => Orientation::Rotate90FlipHorizontal,
            false => Orientation::Rotate90,
        }
    } else if near(180.0) {
        match (matrix[0] < 0, matrix[4] < 0) {
            (true, true) => Orientation::Rotate180,
            (true, false) => Orientation::FlipHorizontal,
            (false, true) => Orientation::FlipVertical,
            (false, false) => Orientation::Upright,
        }
    } else if near(270.0) {
        match matrix[3] < 0 {
            true => Orientation::Rotate270FlipHorizontal,
            false => Orientation::Rotate270,
        }
    } else if near(0.0) && matrix[4] < 0 {
        Orientation::FlipVertical
    } else {
        Orientation::Upright
    }
}

fn turn(matrix: &[i32; 9]) -> Option<f64> {
    let fixed = |cell: i32| f64::from(cell) / 65536.0;
    let across = fixed(matrix[0]).hypot(fixed(matrix[3]));
    let down = fixed(matrix[1]).hypot(fixed(matrix[4]));
    if across == 0.0 || down == 0.0 {
        return None;
    }
    let degrees = (fixed(matrix[1]) / down)
        .atan2(fixed(matrix[0]) / across)
        .to_degrees()
        .round();
    Some(degrees - 360.0 * (degrees / 360.0 + 0.9 / 360.0).floor())
}
