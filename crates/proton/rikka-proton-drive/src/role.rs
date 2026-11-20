use rikka_proton_api::wire_enum;

wire_enum! {
    pub enum Role: default Viewer {
        Viewer = 4,
        Editor = 6,
        Admin = 22,
    }
}

wire_enum! {
    pub enum ShareTarget: default Root {
        Root = 0,
        Folder = 1,
        File = 2,
        Album = 3,
        Photo = 4,
        ProtonVendor = 5,
    }
}
