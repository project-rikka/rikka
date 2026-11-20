pub(crate) fn start_codes(stream: &[u8]) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut index = 0;
    while index + 3 <= stream.len() {
        if stream[index] == 0 && stream[index + 1] == 0 && stream[index + 2] == 1 {
            let start = match index > 0 && stream[index - 1] == 0 {
                true => index - 1,
                false => index,
            };
            found.push((start, index + 3));
            index += 3;
        } else {
            index += 1;
        }
    }
    found
}
