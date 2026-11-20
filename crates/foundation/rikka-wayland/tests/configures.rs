use rikka_wayland::Configures;

enum Step {
    Configured(u32, (u32, u32)),
    Commits((u32, u32), Option<u32>),
}

#[test]
fn commit_acks_newest_matching_configure() {
    use Step::{Commits, Configured};
    let cases: [(&str, &[Step]); 5] = [
        (
            "a buffer at the configured size acks it",
            &[Configured(1, (800, 600)), Commits((800, 600), Some(1))],
        ),
        (
            "a buffer still at the old size acks nothing",
            &[
                Configured(1, (800, 600)),
                Commits((800, 600), Some(1)),
                Configured(2, (900, 600)),
                Commits((800, 600), None),
                Commits((900, 600), Some(2)),
            ],
        ),
        (
            "configures that came together are acked by the newest of the drawn size",
            &[
                Configured(1, (800, 600)),
                Configured(2, (900, 600)),
                Configured(3, (800, 600)),
                Commits((800, 600), Some(3)),
                Commits((900, 600), None),
            ],
        ),
        (
            "an in-flight buffer acks its own configure, leaving the newer one pending",
            &[
                Configured(1, (800, 600)),
                Configured(2, (900, 600)),
                Commits((800, 600), Some(1)),
                Commits((900, 600), Some(2)),
            ],
        ),
        (
            "an acked configure is never acked again",
            &[
                Configured(1, (800, 600)),
                Commits((800, 600), Some(1)),
                Commits((800, 600), None),
            ],
        ),
    ];
    for (case, steps) in cases {
        let mut configures = Configures::new();
        for step in steps {
            match step {
                Configured(serial, size) => configures.push(*serial, *size),
                Commits(size, acked) => {
                    assert_eq!(configures.ack_for_commit(size), *acked, "{case}")
                }
            }
        }
    }
}
