use rikka_shader_graph::{
    Behavior, Category, Hosts, Library, NodeSpec, PortSpec, Symbol, ValueType,
};

pub(crate) const LIGHT: ValueType = ValueType::Record(Symbol::fixed("light"));
pub(crate) const SKY: ValueType = ValueType::Record(Symbol::fixed("sky"));

fn record(id: &'static str, inputs: Vec<PortSpec>, output: PortSpec) -> NodeSpec {
    NodeSpec {
        id,
        title: id,
        category: Category::Light,
        doc: "",
        hosts: Hosts::WORLD,
        inputs,
        outputs: vec![output],
        behavior: Behavior::Record,
    }
}

pub(crate) fn library() -> Library {
    Library::standard()
        .with(NodeSpec {
            id: "filter.blur",
            title: "Blur",
            category: Category::Filter,
            doc: "",
            hosts: Hosts::COMPOSITE,
            inputs: vec![
                PortSpec::required("image", ValueType::Image),
                PortSpec::number("radius", 4.0),
            ],
            outputs: vec![PortSpec::output("image", ValueType::Image)],
            behavior: Behavior::Filter,
        })
        .with(record(
            "world.light",
            vec![
                PortSpec::color("color", 1.0, 1.0, 1.0),
                PortSpec::number("intensity", 1.0),
                PortSpec::empty("profile", ValueType::Texture),
            ],
            PortSpec::output("light", LIGHT),
        ))
        .with(record(
            "world.sky",
            vec![PortSpec::number("hour", 12.0)],
            PortSpec::output("sky", SKY),
        ))
        .with_world(vec![
            PortSpec::empty("sky", SKY),
            PortSpec::empty("lights", LIGHT).many(),
        ])
}
