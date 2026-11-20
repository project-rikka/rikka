pub(crate) fn solve(matrix: &mut [f64], size: usize, rhs: &mut [f64]) -> bool {
    debug_assert_eq!(matrix.len(), size * size);
    debug_assert_eq!(rhs.len(), size);
    for column in 0..size {
        let mut diagonal = matrix[column * size + column];
        for inner in 0..column {
            let entry = matrix[column * size + inner];
            diagonal -= entry * entry;
        }
        if diagonal <= 0.0 || !diagonal.is_finite() {
            return false;
        }
        let diagonal = diagonal.sqrt();
        matrix[column * size + column] = diagonal;
        for row in column + 1..size {
            let mut entry = matrix[row * size + column];
            for inner in 0..column {
                entry -= matrix[row * size + inner] * matrix[column * size + inner];
            }
            matrix[row * size + column] = entry / diagonal;
        }
    }
    for row in 0..size {
        let mut entry = rhs[row];
        for inner in 0..row {
            entry -= matrix[row * size + inner] * rhs[inner];
        }
        rhs[row] = entry / matrix[row * size + row];
    }
    for row in (0..size).rev() {
        let mut entry = rhs[row];
        for inner in row + 1..size {
            entry -= matrix[inner * size + row] * rhs[inner];
        }
        rhs[row] = entry / matrix[row * size + row];
    }
    true
}
