use ui_scene::{Grid, Motion, Table};

#[test]
fn table_operations_follow_js_framework_benchmark() {
    let mut table = Table::with_rows(1000);
    assert_eq!(table.rows.len(), 1000);
    assert_eq!(table.rows[0].id, 1);
    assert_eq!(table.rows[999].id, 1000);
    assert!(
        table
            .rows
            .iter()
            .all(|row| row.label.split(' ').count() == 3)
    );

    let (second, before_last) = (table.rows[1].clone(), table.rows[998].clone());
    table.swap_rows();
    assert_eq!((&table.rows[1], &table.rows[998]), (&before_last, &second));

    table.update();
    let updated: Vec<usize> = (0..1000)
        .filter(|index| table.rows[*index].label.ends_with(" !!!"))
        .collect();
    assert_eq!(updated, (0..1000).step_by(10).collect::<Vec<_>>());

    table.select(table.rows[5].id);
    assert!(table.is_selected(&table.rows[5].clone()));

    let removed = table.rows[1].id;
    table.remove(removed);
    assert_eq!(table.rows.len(), 999);
    assert!(table.rows.iter().all(|row| row.id != removed));

    table.add(1000);
    assert_eq!(table.rows.len(), 1999);
    assert_eq!(table.rows.last().map(|row| row.id), Some(2000));

    table.run(1000);
    assert_eq!(table.rows[0].id, 2001);
    assert_eq!(table.selected, None);

    table.clear();
    assert!(table.rows.is_empty());
}

#[test]
fn grid_matches_the_bui_reference_scene() {
    let cases = [
        (100, 12, 81.0, 86.0, 14.0),
        (1000, 36, 25.0, 26.0, 14.0),
        (3000, 62, 14.0, 14.0, 8.0),
    ];
    for (count, columns, width, height, text) in cases {
        let layout = Grid::new(count, Motion::Anim).layout();
        assert_eq!(
            (
                layout.columns,
                layout.cell_width,
                layout.cell_height,
                layout.text_size
            ),
            (columns, width, height, text),
            "{count} cells"
        );
    }
    let mut one = Grid::new(10, Motion::One);
    one.advance();
    assert_ne!(one.cell(0), Grid::new(10, Motion::One).cell(0));
    assert_eq!(one.cell(1), Grid::new(10, Motion::One).cell(1));
    let mut idle = Grid::new(10, Motion::Idle);
    idle.advance();
    assert_eq!(idle, Grid::new(10, Motion::Idle));
}
