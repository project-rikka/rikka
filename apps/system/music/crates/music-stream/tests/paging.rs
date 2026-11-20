use music_stream::OffsetPaging;

type Page = (usize, Option<usize>);

fn offsets(size: usize, pages: &[Page]) -> Vec<usize> {
    let mut paging = OffsetPaging::new(size);
    let mut asked = Vec::new();
    let mut pages = pages.iter();
    while let Some(offset) = paging.next_offset() {
        asked.push(offset);
        let Some(&(count, total)) = pages.next() else {
            break;
        };
        paging.arrived(count, total);
    }
    asked
}

#[test]
fn offsets_follow_total_and_short_pages() {
    let cases: &[(&str, usize, &[Page], &[usize])] = &[
        (
            "beyond one page",
            500,
            &[(500, Some(1200)), (500, Some(1200)), (200, Some(1200))],
            &[0, 500, 1000],
        ),
        ("one page", 500, &[(30, Some(30))], &[0]),
        (
            "empty page stops",
            500,
            &[(500, Some(2000)), (0, Some(2000))],
            &[0, 500],
        ),
        (
            "short page without total",
            100,
            &[(100, None), (40, None)],
            &[0, 100],
        ),
        (
            "page smaller than asked",
            500,
            &[(100, Some(250)), (100, Some(250)), (50, Some(250))],
            &[0, 100, 200],
        ),
    ];
    for (name, size, pages, expected) in cases {
        assert_eq!(offsets(*size, pages), *expected, "{name}");
    }
}
