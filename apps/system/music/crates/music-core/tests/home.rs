use music_core::{Artist, Artwork, Card, Error, Home, Id, Result, Shelf};

type Outcome = Result<Vec<Shelf>>;

fn artists() -> Shelf {
    Shelf::Artists(
        [Card::Artist(Artist {
            id: Id::new("a"),
            name: "A".to_owned(),
            artwork: Artwork::default(),
        })]
        .into(),
    )
}

fn empty() -> Shelf {
    Shelf::Artists(Vec::new().into())
}

fn refused() -> Error {
    Error::Refused("no".to_owned())
}

#[test]
fn home_keeps_loaded_shelves() {
    let cases: [(Vec<Outcome>, Result<usize>); 4] = [
        (vec![Ok(vec![artists()]), Err(refused())], Ok(1)),
        (vec![Ok(vec![empty()]), Err(refused())], Err(refused())),
        (vec![Ok(vec![empty()])], Ok(0)),
        (
            vec![Ok(vec![artists(), empty()]), Ok(vec![artists()])],
            Ok(2),
        ),
    ];
    for (outcomes, expected) in cases {
        let home = Home::from_outcomes(outcomes).map(|home| home.shelves.len());
        assert_eq!(home, expected);
    }
}
