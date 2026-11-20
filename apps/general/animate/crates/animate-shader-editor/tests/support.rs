use rikka_shader_graph::{
    Behavior, Category, Hosts, Library, NodeSpec, PortSpec, Symbol, ValueType,
};

pub(crate) const LIGHT: ValueType = ValueType::Record(Symbol::fixed("light"));

pub(crate) fn library() -> Library {
    Library::standard()
        .with(NodeSpec {
            id: "world.light",
            title: "Light",
            category: Category::Light,
            doc: "",
            hosts: Hosts::WORLD,
            inputs: vec![
                PortSpec::color("color", 1.0, 1.0, 1.0),
                PortSpec::number("intensity", 1.0).soft(0.0, 10.0),
            ],
            outputs: vec![PortSpec::output("light", LIGHT)],
            behavior: Behavior::Record,
        })
        .with_world(vec![PortSpec::empty("lights", LIGHT).many()])
}
