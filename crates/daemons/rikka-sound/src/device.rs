#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub index: i32,
    pub name: String,
    pub description: String,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub index: i32,
    pub device: i32,
    pub name: String,
    pub description: String,
    pub output: bool,
    pub available: Option<bool>,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub id: u32,
    pub name: String,
    pub title: String,
    pub api: Option<String>,
    pub profiles: Vec<Profile>,
    pub profile: Option<i32>,
    pub routes: Vec<Route>,
}

impl Device {
    pub fn ports(&self, output: bool) -> impl Iterator<Item = &Route> {
        self.routes
            .iter()
            .filter(move |route| route.output == output)
    }
}
