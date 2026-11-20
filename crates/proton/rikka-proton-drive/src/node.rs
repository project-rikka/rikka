use jiff::Timestamp;
use rikka_proton_pgp::PrivateKey;

use crate::attributes::ExtendedAttributes;
use crate::file::File;
use crate::folder::Folder;
use crate::hash_key::HashKey;
use crate::identifier::LinkId;
use crate::link::Link;
use crate::node_keys::NodeKeys;
use crate::share_keys::ShareKeys;
use crate::signatures::Signatures;

#[derive(Debug, Clone)]
pub(crate) struct NodeParts {
    pub(crate) link: Link,
    pub(crate) name: String,
    pub(crate) keys: NodeKeys,
    pub(crate) signatures: Signatures,
    pub(crate) attributes: Option<ExtendedAttributes>,
}

#[derive(Debug, Clone)]
pub enum Node {
    Folder(Folder),
    File(File),
}

impl Node {
    #[must_use]
    pub fn id(&self) -> &LinkId {
        self.link().id()
    }

    #[must_use]
    pub fn parent_id(&self) -> Option<&LinkId> {
        self.link().parent_id()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Node::Folder(folder) => folder.name(),
            Node::File(file) => file.name(),
        }
    }

    #[must_use]
    pub fn link(&self) -> &Link {
        match self {
            Node::Folder(folder) => folder.link(),
            Node::File(file) => file.link(),
        }
    }

    #[must_use]
    pub fn keys(&self) -> &NodeKeys {
        match self {
            Node::Folder(folder) => folder.keys(),
            Node::File(file) => file.keys(),
        }
    }

    #[must_use]
    pub fn signatures(&self) -> &Signatures {
        match self {
            Node::Folder(folder) => folder.signatures(),
            Node::File(file) => file.signatures(),
        }
    }

    #[must_use]
    pub fn attributes(&self) -> Option<&ExtendedAttributes> {
        match self {
            Node::Folder(folder) => folder.attributes(),
            Node::File(file) => file.attributes(),
        }
    }

    #[must_use]
    pub fn modified_at(&self) -> Option<Timestamp> {
        self.link().modified_at()
    }

    #[must_use]
    pub fn is_folder(&self) -> bool {
        matches!(self, Node::Folder(_))
    }

    #[must_use]
    pub fn is_file(&self) -> bool {
        matches!(self, Node::File(_))
    }

    #[must_use]
    pub fn as_folder(&self) -> Option<&Folder> {
        match self {
            Node::Folder(folder) => Some(folder),
            Node::File(_) => None,
        }
    }

    #[must_use]
    pub fn as_file(&self) -> Option<&File> {
        match self {
            Node::File(file) => Some(file),
            Node::Folder(_) => None,
        }
    }

    #[must_use]
    pub fn into_folder(self) -> Option<Folder> {
        match self {
            Node::Folder(folder) => Some(folder),
            Node::File(_) => None,
        }
    }

    #[must_use]
    pub fn into_file(self) -> Option<File> {
        match self {
            Node::File(file) => Some(file),
            Node::Folder(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Parent<'a> {
    Share(&'a ShareKeys),
    Folder(&'a Folder),
}

impl<'a> Parent<'a> {
    #[must_use]
    pub fn key(self) -> &'a PrivateKey {
        match self {
            Parent::Share(share) => share.key(),
            Parent::Folder(folder) => folder.keys().key(),
        }
    }

    #[must_use]
    pub fn hash_key(self) -> Option<&'a HashKey> {
        match self {
            Parent::Share(_) => None,
            Parent::Folder(folder) => folder.keys().hash_key(),
        }
    }

    #[must_use]
    pub fn id(self) -> Option<&'a LinkId> {
        match self {
            Parent::Share(_) => None,
            Parent::Folder(folder) => Some(folder.link().id()),
        }
    }
}

impl<'a> From<&'a ShareKeys> for Parent<'a> {
    fn from(share: &'a ShareKeys) -> Parent<'a> {
        Parent::Share(share)
    }
}

impl<'a> From<&'a Folder> for Parent<'a> {
    fn from(folder: &'a Folder) -> Parent<'a> {
        Parent::Folder(folder)
    }
}
