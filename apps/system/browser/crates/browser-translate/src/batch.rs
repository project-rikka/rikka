use crate::limits::Limits;

pub(crate) fn batches(strings: &[(usize, &str)], limits: Limits) -> Vec<Vec<usize>> {
    let mut batches = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut chars = 0;
    for (index, string) in strings {
        let length = string.chars().count();
        if !current.is_empty() && (chars + length > limits.chars || current.len() == limits.strings)
        {
            batches.push(std::mem::take(&mut current));
            chars = 0;
        }
        current.push(*index);
        chars += length;
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}
