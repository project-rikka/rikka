use rikka_shader_graph::{
    Arg, Builder, Datum, Env, GraphKind, Input, Layer, Link, NodeKind, Op, Problem, Step,
};

use crate::support::library;

fn kinds(steps: &[Step]) -> Vec<&'static str> {
    steps
        .iter()
        .map(|step| match step {
            Step::Filter(_) => "filter",
            Step::Pixel(_) => "pixel",
        })
        .collect()
}

#[test]
fn pixel_maths_between_filters_becomes_one_pass_each() {
    let library = library();
    let mut builder = Builder::new(GraphKind::Composite, &library);
    let scene = builder.layer(Layer::Image);
    let bright = builder.multiply(&scene, 2.0);
    let blur = builder.external("filter.blur", [("image", bright.into())]);
    let blurred = builder.out(blur, "image");
    let sum = builder.add(blurred, &scene);
    builder.output([("image", sum.into())]);
    let graph = builder.finish().expect("a valid composite");
    let plan = graph.composite(&library).expect("it plans");

    assert_eq!(kinds(&plan.steps), ["pixel", "filter", "pixel"]);
    assert_eq!(plan.output, Link::Step(2));
    let Step::Filter(filter) = &plan.steps[1] else {
        panic!("the blur is a filter step");
    };
    assert_eq!(filter.images, [("image".to_owned(), Link::Step(0))]);
    let Step::Pixel(last) = &plan.steps[2] else {
        panic!("the sum is a pixel pass");
    };
    let read: Vec<Link> = last.images.iter().map(|(_, link)| *link).collect();
    assert!(read.contains(&Link::Step(1)) && read.contains(&Link::Layer(Layer::Image)));
    assert_ne!(
        match &plan.steps[0] {
            Step::Pixel(first) => first.key,
            Step::Filter(_) => 0,
        },
        last.key
    );
}

#[test]
fn images_wired_straight_through_need_no_pass() {
    let library = library();
    let mut builder = Builder::new(GraphKind::Composite, &library);
    let scene = builder.layer(Layer::Image);
    let blur = builder.external("filter.blur", [("image", scene.into())]);
    let blurred = builder.out(blur, "image");
    builder.output([("image", blurred.into())]);
    let plan = builder
        .finish()
        .expect("a valid composite")
        .composite(&library)
        .expect("it plans");

    assert_eq!(kinds(&plan.steps), ["filter"]);
    assert_eq!(plan.output, Link::Step(0));
}

#[test]
fn filter_settings_take_one_value_per_frame() {
    let library = library();
    let mut builder = Builder::new(GraphKind::Composite, &library);
    let scene = builder.layer(Layer::Image);
    let across = builder.input(Input::Uv);
    let across = builder.swizzle(across, "x");
    let blur = builder.external(
        "filter.blur",
        [("image", scene.into()), ("radius", across.into())],
    );
    let blurred = builder.out(blur, "image");
    builder.output([("image", blurred.into())]);
    let error = builder.finish().expect_err("a per-pixel radius is refused");
    assert!(
        error
            .errors()
            .iter()
            .any(|error| matches!(error.problem, Problem::Unavailable { .. }))
    );
}

#[test]
fn filter_settings_follow_time_and_overridden_sockets() {
    let library = library();
    let mut builder = Builder::new(GraphKind::Composite, &library);
    let scene = builder.layer(Layer::Image);
    let time = builder.input(Input::Time);
    let scale = builder.node(NodeKind::Op(Op::Multiply));
    builder.set(scale, "a", time).set(scale, "b", 2.0);
    let radius = builder.out(scale, "value");
    let blur = builder.external(
        "filter.blur",
        [("image", scene.into()), ("radius", radius.into())],
    );
    let blurred = builder.out(blur, "image");
    builder.output([("image", blurred.into())]);
    let graph = builder.finish().expect("a valid composite");
    let plan = graph.composite(&library).expect("it plans");

    let radius_at = |time: f32, factor: Option<f32>| {
        let sockets = |at: &rikka_shader_graph::SocketRef| match (factor, at.node == scale) {
            (Some(factor), true) => Some(factor.into()),
            _ => graph.socket(at).cloned(),
        };
        let records = plan.evaluate(&Env {
            time,
            sockets: &sockets,
        });
        match records[0]
            .as_ref()
            .and_then(|record| record.field("radius"))
        {
            Some(Datum::Number(value)) => value.component(0),
            other => panic!("the radius is a number, not {other:?}"),
        }
    };
    assert_eq!(radius_at(3.0, None), 6.0);
    assert_eq!(radius_at(3.0, Some(5.0)), 15.0);
}

#[test]
fn sampling_reads_a_filtered_image_at_any_uv() {
    let library = library();
    let mut builder = Builder::new(GraphKind::Composite, &library);
    let scene = builder.layer(Layer::Image);
    let blur = builder.external("filter.blur", [("image", scene.into())]);
    let blurred = builder.out(blur, "image");
    let uv = builder.input(Input::Uv);
    let shifted = builder.add(uv, Arg::from([0.1, 0.0]));
    let sample = builder.with(
        NodeKind::Sample,
        [("image", blurred.into()), ("uv", shifted.into())],
    );
    let sampled = builder.out(sample, "rgba");
    builder.output([("image", sampled.into())]);
    let plan = builder
        .finish()
        .expect("a valid composite")
        .composite(&library)
        .expect("it plans");

    let Step::Pixel(pass) = &plan.steps[1] else {
        panic!("the sample is a pixel pass");
    };
    assert_eq!(pass.images, [("i0".to_owned(), Link::Step(0))]);
    assert!(pass.source.contains("rikka_effect_sample(push.i0, "));
}
