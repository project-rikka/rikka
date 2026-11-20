use rikka_spirv::{
    Error, Instruction, bytes, instructions, invariant_position, literal_string, words,
};

const MAGIC: u32 = 0x0723_0203;
const OP_NAME: u32 = 5;
const OP_DECORATE: u32 = 71;
const BUILT_IN: u32 = 11;
const INVARIANT: u32 = 18;
const POSITION: u32 = 0;

fn module(body: &[&[u32]]) -> Vec<u32> {
    let mut words = vec![MAGIC, 0x0001_0600, 0, 100, 0];
    for instruction in body {
        words.push(((instruction.len() as u32) << 16) | instruction[0]);
        words.extend_from_slice(&instruction[1..]);
    }
    words
}

#[test]
fn instructions_are_walked_with_their_operands_and_ends() {
    let name = u32::from_le_bytes(*b"main");
    let words = module(&[
        &[OP_NAME, 7, name, 0],
        &[OP_DECORATE, 7, BUILT_IN, POSITION],
    ]);
    let mut walked = instructions(&words).expect("a module");
    assert_eq!(walked.version(), 0x0001_0600);
    let named = walked.next().expect("a name").expect("well formed");
    assert_eq!(
        named,
        Instruction {
            opcode: OP_NAME,
            operands: &[7, name, 0],
            end: 9,
        }
    );
    assert_eq!(literal_string(&named.operands[1..]), ("main".to_owned(), 2));
    assert_eq!(
        walked.next().expect("a decoration").map(|found| found.end),
        Ok(13)
    );
    assert_eq!(walked.next(), None);
}

#[test]
fn malformed_modules_are_refused() {
    assert_eq!(instructions(&[MAGIC, 0, 0]).err(), Some(Error::Header));
    assert_eq!(instructions(&[1, 2, 3, 4, 5]).err(), Some(Error::Header));
    let mut cut = module(&[&[OP_DECORATE, 7, BUILT_IN, POSITION]]);
    cut.pop();
    let mut walked = instructions(&cut).expect("a header");
    assert_eq!(walked.next(), Some(Err(Error::Truncated { at: 5 })));
    assert_eq!(walked.next(), None);
    let mut empty = module(&[]);
    empty.push(0);
    assert_eq!(
        instructions(&empty).expect("a header").next(),
        Some(Err(Error::Truncated { at: 5 }))
    );
    assert_eq!(words(&[1, 2, 3, 4, 5]), Err(Error::Length(5)));
}

#[test]
fn position_is_decorated_invariant_once() {
    let mut words = module(&[
        &[OP_NAME, 7, 0],
        &[OP_DECORATE, 7, BUILT_IN, POSITION],
        &[OP_NAME, 8, 0],
    ]);
    assert!(invariant_position(&mut words));
    let expected = module(&[
        &[OP_NAME, 7, 0],
        &[OP_DECORATE, 7, BUILT_IN, POSITION],
        &[OP_DECORATE, 7, INVARIANT],
        &[OP_NAME, 8, 0],
    ]);
    assert_eq!(words, expected);
    assert!(invariant_position(&mut words));
    assert_eq!(words, expected);
    assert!(!invariant_position(&mut module(&[&[OP_NAME, 7, 0]])));
}

#[test]
fn words_and_bytes_convert_both_ways() {
    let module = module(&[&[OP_NAME, 7, 0]]);
    assert_eq!(words(&bytes(&module)), Ok(module));
}
