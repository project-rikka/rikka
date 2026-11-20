use rikka_proton_key_transparency::{Failure, Identifier, Realm, VrfKey};

#[test]
fn draft_10_edwards25519_vectors() {
    let vectors = [
        (
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            "",
            "8657106690b5526245a92b003bb079ccd1a92130477671f6fc01ad16f26f723f5e8bd1839b414219e8626d393787a192241fc442e6569e96c462f62b8079b9ed83ff2ee21c90c7c398802fdeebea4001",
            "90cf1df3b703cce59e2a35b925d411164068269d7b2d29f3301c03dd757876ff66b71dda49d2de59d03450451af026798e8f81cd2e333de5cdf4f3e140fdd8ae",
        ),
        (
            "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
            "72",
            "f3141cd382dc42909d19ec5110469e4feae18300e94f304590abdced48aed593f7eaf3eb2f1a968cba3f6e23b386aeeaab7b1ea44a256e811892e13eeae7c9f6ea8992557453eac11c4d5476b1f35a08",
            "eb4440665d3891d668e7e0fcaf587f1b4bd7fbfe99d0eb2211ccec90496310eb5e33821bc613efb94db5e5b54c70a848a0bef4553a41befc57663b56373a5031",
        ),
        (
            "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
            "af82",
            "9bc0f79119cc5604bf02d23b4caede71393cedfbb191434dd016d30177ccbf80e29dc513c01c3a980e0e545bcd848222d08a6c3e3665ff5a4cab13a643bef812e284c6b2ee063a2cb4f456794723ad0a",
            "645427e5d00c62a23fb703732fa5d892940935942101e456ecca7bb217c61c452118fec1219202a0edcf038bb6373241578be7217ba85a2687f7a0310b2df19f",
        ),
    ];

    for (key, alpha, proof, output) in vectors {
        let key = VrfKey::from_hex(key).unwrap();
        let alpha = rikka_types::hex::decode(alpha).unwrap();
        let proof = rikka_types::hex::decode(proof).unwrap();

        let beta = key.verify(&alpha, &proof).unwrap();
        assert_eq!(rikka_types::hex::encode(beta), output);

        let mut tampered = proof.clone();
        tampered[40] ^= 1;
        assert_eq!(key.verify(&alpha, &tampered), Err(Failure::VrfProof));
        assert_eq!(key.verify(b"other", &proof), Err(Failure::VrfProof));
    }
}

#[test]
fn development_key_verifies_proofs() {
    let served = [
        (
            "marintest@proton.black",
            "f4ebc424d55cddc6afe1df56ae0cde41d61b5dbe47e25432db4622ec83fade60ececbe11b88884fa81893afdaf522ed99cbd8ebf8c4140015f3d993b338e01014edfede032fb0f74ecddfff0d98df50f",
        ),
        (
            "mttestpm@gmail.com",
            "50971599f49155bb070db21387a5f23c111abd89ada6f967fc413d67f6afd6a9d65541fd7d8a2ecb8533d76a2938daff87539c3e127ce53ea8c3606b33672825ed935e81d41d080f96873cd48efc9b07",
        ),
    ];

    for (email, proof) in served {
        let identifier = Identifier::address(email).unwrap();
        let proof = rikka_types::hex::decode(proof).unwrap();
        let alpha = identifier.as_str().as_bytes();

        assert!(Realm::atlas().vrf_key().verify(alpha, &proof).is_ok());
        assert_eq!(
            Realm::production().vrf_key().verify(alpha, &proof),
            Err(Failure::VrfProof)
        );
    }
}

#[test]
fn vrf_inputs_are_canonical_addresses() {
    let cases = [
        (
            "normalize_this-test.proton@proton.black",
            false,
            "normalizethistestproton@proton.black",
        ),
        (
            "normalize_this-test.proton+alias_to_remove@proton.black",
            false,
            "normalizethistestproton@proton.black",
        ),
        ("Some.One@Proton.Me", false, "someone@proton.me"),
        (
            "normalize_this-test.proton@Proton.Black",
            true,
            "@proton.black",
        ),
    ];
    for (email, catch_all, expected) in cases {
        let identifier = if catch_all {
            Identifier::catch_all(email)
        } else {
            Identifier::address(email)
        }
        .unwrap();
        assert_eq!(identifier.as_str(), expected);
    }
    assert!(Identifier::address("not an email").is_err());
}
