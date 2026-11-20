use divan::{AllocProfiler, Bencher};
use ui_frontends::{Egui, Frontend, Gpui, GpuiFast, Iced, Rikka, Screen};
use ui_scene::{CELL_COUNTS, Grid, Motion, Table};

#[global_allocator]
static ALLOCATOR: AllocProfiler = AllocProfiler::system();

const ROWS: usize = 1_000;

const MANY_ROWS: usize = 10_000;

const OPERATION_SAMPLES: u32 = 40;

fn main() {
    divan::main();
}

fn warmed<F: Frontend>(rows: usize) -> F {
    let mut frontend = F::open(F::host(), Screen::Table(Table::with_rows(ROWS)));
    frontend.update(|screen| *screen = Screen::Table(Table::with_rows(rows)));
    frontend
}

fn operation<F: Frontend>(bencher: Bencher, rows: usize, change: fn(&mut Table)) {
    bencher
        .with_inputs(|| warmed::<F>(rows))
        .bench_local_refs(|frontend| frontend.update(|screen| change(screen.table())));
}

fn second_row(table: &Table) -> usize {
    table.rows[1].id
}

#[divan::bench(types = [Rikka, Egui, Iced, Gpui, GpuiFast], sample_size = 1, sample_count = OPERATION_SAMPLES)]
fn create_rows<F: Frontend>(bencher: Bencher) {
    operation::<F>(bencher, 0, |table| table.run(ROWS));
}

#[divan::bench(types = [Rikka, Egui, Iced, Gpui, GpuiFast], sample_size = 1, sample_count = OPERATION_SAMPLES)]
fn replace_all_rows<F: Frontend>(bencher: Bencher) {
    operation::<F>(bencher, ROWS, |table| table.run(ROWS));
}

#[divan::bench(types = [Rikka, Egui, Iced, Gpui, GpuiFast], sample_size = 1, sample_count = OPERATION_SAMPLES)]
fn partial_update<F: Frontend>(bencher: Bencher) {
    operation::<F>(bencher, ROWS, Table::update);
}

#[divan::bench(types = [Rikka, Egui, Iced, Gpui, GpuiFast], sample_size = 1, sample_count = OPERATION_SAMPLES)]
fn select_row<F: Frontend>(bencher: Bencher) {
    operation::<F>(bencher, ROWS, |table| table.select(second_row(table)));
}

#[divan::bench(types = [Rikka, Egui, Iced, Gpui, GpuiFast], sample_size = 1, sample_count = OPERATION_SAMPLES)]
fn swap_rows<F: Frontend>(bencher: Bencher) {
    operation::<F>(bencher, ROWS, Table::swap_rows);
}

#[divan::bench(types = [Rikka, Egui, Iced, Gpui, GpuiFast], sample_size = 1, sample_count = OPERATION_SAMPLES)]
fn remove_row<F: Frontend>(bencher: Bencher) {
    operation::<F>(bencher, ROWS, |table| table.remove(second_row(table)));
}

#[divan::bench(types = [Rikka, Egui, Iced, Gpui, GpuiFast], sample_size = 1, sample_count = OPERATION_SAMPLES)]
fn create_many_rows<F: Frontend>(bencher: Bencher) {
    operation::<F>(bencher, 0, |table| table.run(MANY_ROWS));
}

#[divan::bench(types = [Rikka, Egui, Iced, Gpui, GpuiFast], sample_size = 1, sample_count = OPERATION_SAMPLES)]
fn append_rows<F: Frontend>(bencher: Bencher) {
    operation::<F>(bencher, ROWS, |table| table.add(ROWS));
}

#[divan::bench(types = [Rikka, Egui, Iced, Gpui, GpuiFast], sample_size = 1, sample_count = OPERATION_SAMPLES)]
fn clear_rows<F: Frontend>(bencher: Bencher) {
    operation::<F>(bencher, ROWS, Table::clear);
}

fn cells<F: Frontend>(bencher: Bencher, count: usize, motion: Motion) {
    let mut frontend = F::open(F::host(), Screen::Grid(Grid::new(count, motion)));
    bencher.bench_local(|| frontend.update(|screen| screen.grid().advance()));
}

#[divan::bench(types = [Rikka, Egui, Iced, Gpui, GpuiFast], args = CELL_COUNTS)]
fn cells_anim<F: Frontend>(bencher: Bencher, count: usize) {
    cells::<F>(bencher, count, Motion::Anim);
}

#[divan::bench(types = [Rikka, Egui, Iced, Gpui, GpuiFast], args = CELL_COUNTS)]
fn cells_one<F: Frontend>(bencher: Bencher, count: usize) {
    cells::<F>(bencher, count, Motion::One);
}
