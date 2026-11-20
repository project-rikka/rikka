const BASE: u32 = 36;
const T_MIN: u32 = 1;
const T_MAX: u32 = 26;
const SKEW: u32 = 38;
const DAMP: u32 = 700;
const INITIAL_BIAS: u32 = 72;
const INITIAL_N: u32 = 128;

pub(crate) fn encode(label: &str) -> String {
    let input: Vec<u32> = label.chars().map(u32::from).collect();
    let mut output: String = label.chars().filter(char::is_ascii).collect();
    let basic = output.len();
    if basic > 0 {
        output.push('-');
    }
    let mut n = INITIAL_N;
    let mut delta: u32 = 0;
    let mut bias = INITIAL_BIAS;
    let mut handled = basic;
    while handled < input.len() {
        let next = input
            .iter()
            .copied()
            .filter(|&point| point >= n)
            .min()
            .expect("an unhandled code point remains");
        delta += (next - n) * (handled as u32 + 1);
        n = next;
        for &point in &input {
            if point < n {
                delta += 1;
            }
            if point == n {
                let mut q = delta;
                let mut k = BASE;
                loop {
                    let threshold = if k <= bias {
                        T_MIN
                    } else if k >= bias + T_MAX {
                        T_MAX
                    } else {
                        k - bias
                    };
                    if q < threshold {
                        break;
                    }
                    output.push(digit(threshold + (q - threshold) % (BASE - threshold)));
                    q = (q - threshold) / (BASE - threshold);
                    k += BASE;
                }
                output.push(digit(q));
                bias = adapt(delta, handled as u32 + 1, handled == basic);
                delta = 0;
                handled += 1;
            }
        }
        delta += 1;
        n += 1;
    }
    output
}

fn adapt(delta: u32, points: u32, first: bool) -> u32 {
    let mut delta = if first { delta / DAMP } else { delta / 2 };
    delta += delta / points;
    let mut k = 0;
    while delta > ((BASE - T_MIN) * T_MAX) / 2 {
        delta /= BASE - T_MIN;
        k += BASE;
    }
    k + (BASE - T_MIN + 1) * delta / (delta + SKEW)
}

fn digit(value: u32) -> char {
    let byte = match value {
        0..=25 => b'a' + value as u8,
        _ => b'0' + (value - 26) as u8,
    };
    char::from(byte)
}
