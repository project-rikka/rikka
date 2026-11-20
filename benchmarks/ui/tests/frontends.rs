use ui_frontends::{Egui, Frontend, Gpui, GpuiFast, Iced, Rikka, Screen};
use ui_scene::{Grid, Motion, Table};

fn renders_changes<F: Frontend>() {
    let mut table = F::open(F::host(), Screen::Table(Table::with_rows(100)));
    table.update(|screen| screen.table().update());
    table.update(|screen| {
        let id = screen.table().rows[1].id;
        screen.table().select(id);
    });
    table.update(|screen| screen.table().run(1000));
    let mut grid = F::open(F::host(), Screen::Grid(Grid::new(100, Motion::Anim)));
    grid.update(|screen| screen.grid().advance());
}

#[test]
fn frontends_render_table_and_grid_changes() {
    renders_changes::<Rikka>();
    renders_changes::<Egui>();
    renders_changes::<Iced>();
    renders_changes::<Gpui>();
    renders_changes::<GpuiFast>();
}
