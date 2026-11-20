use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

use music_core::{Error, Result};
use music_stream::{Fetcher, Plan, Spool};

const LENGTH: usize = 40_000;
const REFUSED: usize = 403;

fn body(index: usize) -> Vec<u8> {
    (0..LENGTH).map(|byte| (byte * 7 + index) as u8).collect()
}

async fn serve(listener: TcpListener, cut: Arc<AtomicUsize>) {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let cut = Arc::clone(&cut);
        tokio::spawn(async move {
            let mut reader = BufReader::new(&mut socket);
            let mut first = String::new();
            reader.read_line(&mut first).await.unwrap();
            let mut range = None;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                if line.trim().is_empty() {
                    break;
                }
                if let Some(requested) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                    let (from, to) = requested.trim().split_once('-').unwrap();
                    range = Some((from.parse::<usize>().unwrap(), to.parse::<usize>().ok()));
                }
            }
            let index: usize = first
                .split('/')
                .nth(1)
                .unwrap()
                .split(' ')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            let whole = body(index);
            let (status, rest) = match range {
                _ if index == REFUSED => {
                    cut.fetch_add(1, Ordering::SeqCst);
                    ("403 Forbidden", &[][..])
                }
                Some((from, _)) if from >= whole.len() => ("416 Range Not Satisfiable", &[][..]),
                Some((from, to)) => {
                    let to = to.map_or(whole.len(), |to| (to + 1).min(whole.len()));
                    ("206 Partial Content", &whole[from..to])
                }
                None => ("200 OK", &whole[..]),
            };
            let head = format!(
                "HTTP/1.1 {status}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                rest.len()
            );
            socket.write_all(head.as_bytes()).await.unwrap();
            let cutting = index == 2 && cut.fetch_add(1, Ordering::SeqCst) == 0;
            let sent = match cutting {
                true => &rest[..rest.len() / 3],
                false => rest,
            };
            let _ = socket.write_all(sent).await;
            let _ = socket.shutdown().await;
        });
    }
}

fn fetched(fetcher: Fetcher, plan: impl FnOnce(u16) -> Plan) -> (Result<()>, Vec<u8>, usize) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let folder = tempfile::tempdir().unwrap();
    let mut spool = Spool::create(&folder.path().join("track")).unwrap();
    let mut stored = spool.stored().unwrap();
    let cut = Arc::new(AtomicUsize::new(0));
    let answer = runtime.block_on(async {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(serve(listener, Arc::clone(&cut)));
        fetcher.fetch(plan(port), &mut spool).await
    });
    drop(spool);
    let mut received = Vec::new();
    let _ = stored.read_to_end(&mut received);
    (answer, received, cut.load(Ordering::SeqCst))
}

fn framed(length: u64) -> Result<Vec<u8>> {
    Ok(u32::try_from(length).unwrap().to_be_bytes().to_vec())
}

#[test]
fn framed_pieces_arrive_in_order_after_cut() {
    let (answer, received, cuts) = fetched(Fetcher::new("test").ahead(3).tries(3), |port| {
        (1..=3)
            .fold(Plan::new().lead(b"head".to_vec()), |plan, index| {
                plan.piece(format!("http://127.0.0.1:{port}/{index}"), None)
            })
            .framed(framed)
    });

    answer.unwrap();
    assert_eq!(cuts, 2, "piece 2 was asked for again");
    let expected: Vec<u8> = (1..=3)
        .flat_map(|index| [framed(LENGTH as u64).unwrap(), body(index)].concat())
        .collect();
    assert_eq!(received, [b"head".to_vec(), expected].concat());
}

#[test]
fn cut_piece_fails_without_retries() {
    let (answer, _, _) = fetched(Fetcher::new("test").ahead(2), |port| {
        (1..=3).fold(Plan::new(), |plan, index| {
            plan.piece(
                format!("http://127.0.0.1:{port}/{index}"),
                Some(LENGTH as u64),
            )
        })
    });

    assert!(answer.is_err());
}

#[test]
fn unknown_length_ends_at_short_range() {
    let (answer, received, _) = fetched(Fetcher::new("test").ahead(4), |port| {
        Plan::file(format!("http://127.0.0.1:{port}/5"), None)
    });

    answer.unwrap();
    assert_eq!(received, body(5));
}

#[test]
fn refused_piece_fails_once_as_refused() {
    let (answer, _, requests) = fetched(Fetcher::new("test").tries(3), |port| {
        Plan::file(
            format!("http://127.0.0.1:{port}/{REFUSED}"),
            Some(LENGTH as u64),
        )
    });

    assert!(matches!(answer, Err(Error::Refused(_))), "{answer:?}");
    assert_eq!(requests, 1, "a refusal is not retried");
}
