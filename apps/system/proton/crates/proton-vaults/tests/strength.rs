use proton_vaults::Strength;

#[test]
fn strength_reflects_guessability() {
    let cases = [
        ("", Strength::Vulnerable),
        ("hunter2", Strength::Vulnerable),
        ("password", Strength::Vulnerable),
        ("aaaaaaaaaaaa", Strength::Vulnerable),
        ("Tr0ub4dor&3x", Strength::Weak),
        ("7Zq!wk3Lm#Pv2Rt%Xb", Strength::Strong),
        ("correct-horse-battery-staple-42", Strength::Strong),
    ];

    for (password, strength) in cases {
        assert_eq!(Strength::estimate(password), strength, "{password:?}");
    }
}
