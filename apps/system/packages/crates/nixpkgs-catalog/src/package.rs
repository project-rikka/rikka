use crate::license::License;
use crate::maintainer::Maintainer;
use crate::team::Team;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub attribute: String,
    pub set: Option<String>,
    pub name: String,
    pub version: String,
    pub description: String,
    pub about: String,
    pub main_program: Option<String>,
    pub outputs: Vec<String>,
    pub platforms: Vec<String>,
    pub unfree: bool,
    pub licenses: Vec<License>,
    pub maintainers: Vec<Maintainer>,
    pub teams: Vec<Team>,
    pub homepages: Vec<String>,
    pub position: Option<String>,
}
