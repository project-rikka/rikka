use installer_core::{
    Answers, ByteSize, Catalog, Disk, DiskKind, GeneratedFile, Gpu, GpuVendor, Keyboard, Locale,
    Secret, generate,
};

pub(crate) const CATALOG: &str = r#"{
  "base": {"name": "Rikka desktop", "summary": "The session", "offer": "required", "version": "0.1.0"},
  "browser": {"name": "Browser", "summary": "Browse the web", "offer": "selected", "version": "0.1.0"},
  "files": {"name": "Files", "summary": "Browse files", "offer": "required", "version": "0.1.0"},
  "modeler": {"name": "Modeler", "summary": "Model in 3D", "offer": "offered", "version": "0.2.0"},
  "paint": {"name": "Paint", "summary": "Edit pictures", "offer": "selected", "version": "0.1.0"}
}"#;

pub(crate) fn catalog() -> Catalog {
    Catalog::parse(CATALOG).expect("the fixture parses")
}

pub(crate) fn disk(size: ByteSize, path: &str) -> Disk {
    Disk {
        name: path.rsplit('/').next().unwrap_or_default().to_owned(),
        path: path.to_owned(),
        size,
        model: "Samsung SSD 980 PRO".to_owned(),
        kind: DiskKind::Nvme,
        removable: false,
        read_only: false,
        in_use: false,
    }
}

pub(crate) fn answers() -> Answers {
    let mut answers = Answers {
        locale: Locale::find("ja_JP.UTF-8").expect("entry should be in the catalog"),
        keyboard: Keyboard::find("Japanese (JIS 106)").expect("entry should be in the catalog"),
        timezone: "Asia/Tokyo".to_owned(),
        hostname: "rikka".to_owned(),
        username: "eldodebug".to_owned(),
        password: Secret::from("correct horse battery"),
        disk: Some(disk(ByteSize::from_gib(476), "/dev/nvme0n1")),
        ..Answers::default()
    };
    answers.offer(catalog());
    answers
}

pub(crate) fn gpu(vendor: GpuVendor, device: u16, kernel_module: &str) -> Gpu {
    Gpu {
        vendor,
        device,
        model: format!("{vendor:?} {device:#06x}"),
        kernel_module: kernel_module.to_owned(),
    }
}

pub(crate) fn generated(answers: &Answers) -> Vec<GeneratedFile> {
    generate(answers).expect("disk should fit the layout")
}

pub(crate) fn file<'a>(files: &'a [GeneratedFile], name: &str) -> &'a str {
    &files
        .iter()
        .find(|file| file.path.ends_with(&format!("/{name}")))
        .unwrap_or_else(|| panic!("{name} is generated"))
        .contents
}
