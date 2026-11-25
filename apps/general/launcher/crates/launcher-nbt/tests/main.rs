use launcher_nbt::{Compound, Error, List, Tag, read, write};

fn servers_dat() -> Vec<u8> {
    let mut bytes = vec![10, 0, 0, 9, 0, 7];
    bytes.extend_from_slice(b"servers");
    bytes.extend_from_slice(&[10, 0, 0, 0, 1]);
    bytes.extend_from_slice(&[8, 0, 2]);
    bytes.extend_from_slice(b"ip");
    bytes.extend_from_slice(&[0, 15]);
    bytes.extend_from_slice(b"mc.example.com:");
    bytes.extend_from_slice(&[8, 0, 4]);
    bytes.extend_from_slice(b"name");
    bytes.extend_from_slice(&[0, 4]);
    bytes.extend_from_slice(b"Home");
    bytes.extend_from_slice(&[1, 0, 11]);
    bytes.extend_from_slice(b"acceptTexts");
    bytes.push(1);
    bytes.extend_from_slice(&[0, 0]);
    bytes
}

#[test]
fn reads_vanilla_server_list() {
    let (name, root) = read(&servers_dat()).expect("servers.dat parses");
    assert_eq!(name, "");
    let servers = root.list("servers").expect("servers list");
    let Tag::Compound(server) = &servers.items()[0] else {
        panic!("a server is a compound");
    };
    assert_eq!(server.string("name"), Some("Home"));
    assert_eq!(server.string("ip"), Some("mc.example.com:"));
    assert_eq!(server.integer("acceptTexts"), Some(1));
}

#[test]
fn write_round_trips() {
    let mut inner = Compound::new();
    inner.insert("text", Tag::String("nul\0 and 🎮 and é".to_owned()));
    inner.insert("longs", Tag::LongArray(vec![i64::MIN, 0, i64::MAX]));
    inner.insert("bytes", Tag::ByteArray(vec![-1, 0, 1]));
    let mut list = List::empty();
    list.push(Tag::Double(1.5)).expect("first item");
    list.push(Tag::Double(-2.0)).expect("same kind");
    assert!(list.push(Tag::Int(3)).is_err());
    let mut root = Compound::new();
    root.insert("Data", Tag::Compound(inner));
    root.insert("values", Tag::List(list));
    root.insert("empty", Tag::List(List::empty()));
    let bytes = write("root", &root).expect("writes");
    assert_eq!(read(&bytes).expect("reads"), ("root".to_owned(), root));
}

#[test]
fn damaged_input_is_refused() {
    let complete = servers_dat();
    for end in 0..complete.len() - 1 {
        assert!(read(&complete[..end]).is_err(), "cut at {end}");
    }
    let mut nested = vec![10, 0, 0];
    for _ in 0..600 {
        nested.extend_from_slice(&[10, 0, 0]);
    }
    assert_eq!(read(&nested), Err(Error::Depth(512)));
    assert_eq!(read(&[8, 0, 0]), Err(Error::Root));
    assert_eq!(
        read(&[10, 0, 0, 7, 0, 0, 255, 255, 255, 255]),
        Err(Error::Length(-1))
    );
}
