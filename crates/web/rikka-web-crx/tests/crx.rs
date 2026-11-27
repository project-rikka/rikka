use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use rikka_web_crx::{Identity, Package, Version};
use sha2::{Digest, Sha256};

const MANIFEST: &[u8] = br#"{"manifest_version": 3, "name": "Probe", "version": "0"}"#;

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn read_varint(bytes: &[u8], at: &mut usize) -> u64 {
    let mut value = 0;
    let mut shift = 0;
    loop {
        let byte = bytes[*at];
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return value;
        }
        shift += 7;
    }
}

fn fields(bytes: &[u8]) -> Vec<(u64, Vec<u8>)> {
    let mut at = 0;
    let mut fields = Vec::new();
    while at < bytes.len() {
        let key = read_varint(bytes, &mut at);
        assert_eq!(key & 7, 2, "every CRX header field is length-delimited");
        let length = read_varint(bytes, &mut at) as usize;
        fields.push((key >> 3, bytes[at..at + length].to_vec()));
        at += length;
    }
    fields
}

fn field(fields: &[(u64, Vec<u8>)], number: u64) -> Vec<u8> {
    fields
        .iter()
        .find(|(field, _)| *field == number)
        .map(|(_, value)| value.clone())
        .unwrap_or_else(|| panic!("field {number} is present"))
}

#[test]
fn crx3_header_proves_the_identity_over_the_archive() {
    let identity = Identity::from_seed(b"probe");
    let package = Package::new()
        .file("manifest.json", MANIFEST.to_vec())
        .file("scripts/page.js", b"console.log(1);".to_vec())
        .stamp(&identity, Version::parse("1.2.3.4").unwrap())
        .unwrap();
    let crx = package.crx(&identity).unwrap();

    assert_eq!(&crx[..4], b"Cr24");
    assert_eq!(read_u32(&crx, 4), 3);
    let header_length = read_u32(&crx, 8) as usize;
    let header = fields(&crx[12..12 + header_length]);
    let archive = &crx[12 + header_length..];

    let proof = fields(&field(&header, 3));
    let public_key = field(&proof, 1);
    let signature = field(&proof, 2);
    let signed_data = field(&header, 10000);
    let crx_id = field(&fields(&signed_data), 1);

    assert_eq!(public_key, identity.public_key());
    assert_eq!(crx_id, Sha256::digest(&public_key)[..16].to_vec());

    let mut message = b"CRX3 SignedData\x00".to_vec();
    message.extend_from_slice(&(signed_data.len() as u32).to_le_bytes());
    message.extend_from_slice(&signed_data);
    message.extend_from_slice(archive);
    let key = VerifyingKey::from_public_key_der(&public_key).unwrap();
    key.verify(&message, &Signature::from_der(&signature).unwrap())
        .expect("the proof signs the documented message");

    let unpacked = Package::from_zip(archive).unwrap();
    assert_eq!(
        unpacked.version().unwrap(),
        Version::parse("1.2.3.4").unwrap()
    );
}

#[test]
fn extension_id_maps_the_key_hash_to_letters() {
    let identity = Identity::from_seed(b"probe");
    let hash = Sha256::digest(identity.public_key());
    let expected: String = hash[..16]
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 15])
        .map(|nibble| char::from(b'a' + nibble))
        .collect();
    assert_eq!(identity.extension_id(), expected);
    assert_eq!(
        identity.extension_id(),
        Identity::from_seed(b"probe").extension_id()
    );
    assert_ne!(identity.extension_id(), Identity::rikka().extension_id());
}

#[test]
fn unpacking_refuses_paths_outside_the_folder() {
    let folder = tempfile::tempdir().unwrap();
    let package = Package::new().file("../escape.js", Vec::new());
    assert!(package.unpack(folder.path()).is_err());
}
