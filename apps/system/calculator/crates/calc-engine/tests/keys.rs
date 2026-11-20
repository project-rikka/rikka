use calc_engine::{Engine, Error, Key, Operator, Unary};

fn press(engine: &mut Engine, keys: &str) {
    for key in keys.chars() {
        engine.press(match key {
            '+' => Key::Operator(Operator::Add),
            '-' => Key::Operator(Operator::Subtract),
            '*' => Key::Operator(Operator::Multiply),
            '/' => Key::Operator(Operator::Divide),
            '=' => Key::Equals,
            '.' => Key::Point,
            '%' => Key::Percent,
            'c' => Key::Clear,
            'e' => Key::ClearEntry,
            '<' => Key::Backspace,
            '~' => Key::Unary(Unary::Negate),
            'r' => Key::Unary(Unary::SquareRoot),
            's' => Key::Unary(Unary::Square),
            'i' => Key::Unary(Unary::Reciprocal),
            digit => Key::Digit(digit.to_digit(10).expect("unknown key") as u8),
        });
    }
}

fn run(keys: &str) -> Engine {
    let mut engine = Engine::default();
    press(&mut engine, keys);
    engine
}

#[test]
fn key_sequences_match_desk_calculator() {
    let cases: [(&str, &str, Option<&str>); 25] = [
        ("", "0", Some("")),
        ("2+3+", "5", None),
        ("2+3+4=", "9", Some("2 + 3 + 4 =")),
        ("2+*3=", "6", Some("2 × 3 =")),
        ("3+2=", "5", None),
        ("3+2==", "7", Some("5 + 2 =")),
        ("3+2===", "9", None),
        ("2+3=7", "7", Some("")),
        ("2+3=7+1=", "8", None),
        ("2+3=*2=", "10", Some("5 × 2 =")),
        ("50+10%", "5", None),
        ("50+10%=", "55", Some("50 + 5 =")),
        ("50*10%", "0.1", None),
        ("50*10%=", "5", None),
        ("10%", "0", None),
        ("9rs", "9", Some("sqr(√(9))")),
        ("5+9r=", "8", Some("5 + √(9) =")),
        ("5~", "-5", Some("")),
        ("5~1", "-51", None),
        ("2+3=~", "-5", Some("negate(5)")),
        ("12<", "1", None),
        ("2+3=<", "5", None),
        ("5+3e", "0", Some("5 +")),
        ("5+3e8=", "13", None),
        ("0.500", "0.500", None),
    ];
    for (keys, display, expression) in cases {
        let engine = run(keys);
        assert_eq!(engine.display(), display, "display after {keys}");
        if let Some(expression) = expression {
            assert_eq!(engine.expression(), expression, "expression after {keys}");
        }
    }
}

#[test]
fn numbers_display_grouped_copy_ungrouped() {
    let cases = [
        ("1234567", "1,234,567", "1234567"),
        (
            "12345678901234567",
            "1,234,567,890,123,456",
            "1234567890123456",
        ),
        ("0.1+0.2=", "0.3", "0.3"),
        ("1/3=", "0.3333333333333333", "0.3333333333333333"),
        ("2/3=", "0.6666666666666666", "0.6666666666666666"),
        ("0.0001*1=", "0.0001", "0.0001"),
        ("100000*100000=*10000000000=", "1e+20", "1e+20"),
        ("12.5<<<<", "0", "0"),
        ("1234567~", "-1,234,567", "-1234567"),
    ];
    for (keys, display, display_ungrouped) in cases {
        let engine = run(keys);
        assert_eq!(engine.display(), display, "display after {keys}");
        assert_eq!(
            engine.display_ungrouped(),
            display_ungrouped,
            "display_ungrouped after {keys}"
        );
    }
}

#[test]
fn error_blocks_keys_until_clear() {
    let mut engine = run("1/0=");
    assert_eq!(engine.error(), Some(Error::DivisionByZero));

    press(&mut engine, "5+7=");
    assert_eq!(engine.error(), Some(Error::DivisionByZero));

    press(&mut engine, "e");
    assert_eq!(engine.error(), None);
    assert_eq!(engine.display(), "0");
    press(&mut engine, "5+7=");
    assert_eq!(engine.display(), "12");
}

#[test]
fn invalid_operations_report_distinct_errors() {
    let cases = [
        ("0/0=", Error::Undefined),
        ("1/0=", Error::DivisionByZero),
        ("0i", Error::DivisionByZero),
        ("1~r", Error::InvalidInput),
    ];
    for (keys, error) in cases {
        assert_eq!(run(keys).error(), Some(error), "after {keys}");
    }
}
