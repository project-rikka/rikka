use rikka_remote_link::{Cauchy, FecError};

fn shards(data: usize, length: usize, seed: u8) -> Vec<Vec<u8>> {
    (0..data)
        .map(|shard| {
            (0..length)
                .map(|byte| {
                    (byte as u8)
                        .wrapping_mul(31)
                        .wrapping_add(shard as u8 ^ seed)
                })
                .collect()
        })
        .collect()
}

#[test]
fn any_data_shards_come_back_from_any_k_shards() {
    for (data, parity, length) in [
        (1, 1, 7),
        (3, 2, 1200),
        (10, 4, 33),
        (100, 30, 64),
        (200, 55, 17),
    ] {
        let originals = shards(data, length, data as u8);
        let coder = Cauchy::new(data, parity).unwrap();
        let sources: Vec<&[u8]> = originals.iter().map(Vec::as_slice).collect();
        let mut parities = vec![Vec::new(); parity];
        coder.encode(&sources, &mut parities).unwrap();
        for first_lost in [0, data.saturating_sub(parity)] {
            let mut received: Vec<Option<Vec<u8>>> = originals
                .iter()
                .cloned()
                .chain(parities.iter().cloned())
                .map(Some)
                .collect();
            for lost in received.iter_mut().skip(first_lost).take(parity) {
                *lost = None;
            }
            coder.reconstruct(&mut received).unwrap();
            for (index, original) in originals.iter().enumerate() {
                assert_eq!(
                    received[index].as_ref(),
                    Some(original),
                    "data {data} parity {parity}: shard {index} differs"
                );
            }
        }
    }
}

#[test]
fn parity_matches_the_documented_cauchy_matrix() {
    let data = [vec![0x01_u8], vec![0x02], vec![0x80]];
    let sources: Vec<&[u8]> = data.iter().map(Vec::as_slice).collect();
    let mut parity = vec![Vec::new(); 1];
    Cauchy::new(3, 1)
        .unwrap()
        .encode(&sources, &mut parity)
        .unwrap();
    let mul = |mut a: u8, mut b: u8| {
        let mut product = 0_u8;
        while b != 0 {
            if b & 1 != 0 {
                product ^= a;
            }
            let carry = a & 0x80 != 0;
            a <<= 1;
            if carry {
                a ^= 0x1D;
            }
            b >>= 1;
        }
        product
    };
    let inverse = |value: u8| {
        (1..=255_u8)
            .find(|candidate| mul(value, *candidate) == 1)
            .unwrap()
    };
    let expected = (0..3).fold(0_u8, |sum, column| {
        sum ^ mul(inverse(3 ^ column as u8), data[column][0])
    });
    assert_eq!(parity[0], vec![expected]);
}

#[test]
fn too_few_shards_are_refused() {
    let coder = Cauchy::new(4, 2).unwrap();
    let mut received = vec![
        Some(vec![0; 8]),
        None,
        None,
        None,
        Some(vec![0; 8]),
        Some(vec![0; 8]),
    ];
    assert_eq!(
        coder.reconstruct(&mut received),
        Err(FecError::Short { have: 3, need: 4 })
    );
    assert!(matches!(
        Cauchy::new(200, 56),
        Err(FecError::TooMany { .. })
    ));
}
