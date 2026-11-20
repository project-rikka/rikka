use std::io::{Read, Write};
use std::path::Path;

use crate::address::Address;
use crate::error::{Action, Error, Result};
use crate::item::Item;

pub trait Sink: Write + Send {
    fn commit(self: Box<Self>) -> std::io::Result<()>;
}

pub type ReadSink<'a, L> = dyn FnMut(usize, Result<&mut dyn Read, L>) -> Result<(), L> + 'a;

pub trait Store: Send + Sync + 'static {
    type Location: Address;
    type Item: Item<Location = Self::Location>;

    fn stat(&self, at: &Self::Location) -> Result<Self::Item, Self::Location>;

    fn walk(&self, at: &Self::Location) -> Result<Vec<Self::Item>, Self::Location>;

    fn create_directory(&self, at: &Self::Location) -> Result<(), Self::Location>;

    fn delete(&self, at: &Self::Location) -> Result<(), Self::Location>;

    fn trash(&self, at: &Self::Location) -> Result<Self::Location, Self::Location>;

    fn restore(&self, at: &Self::Location) -> Result<Self::Location, Self::Location>;

    fn read(&self, at: &Self::Location) -> Result<Box<dyn Read + Send>, Self::Location>;

    fn read_each(
        &self,
        at: &[Self::Location],
        take: &mut ReadSink<'_, Self::Location>,
    ) -> Result<(), Self::Location> {
        for (position, location) in at.iter().enumerate() {
            match self.read(location) {
                Ok(mut source) => take(position, Ok(&mut source))?,
                Err(error) => take(position, Err(error))?,
            }
        }
        Ok(())
    }

    fn write(&self, at: &Self::Location) -> Result<Box<dyn Sink>, Self::Location>;

    fn link(&self, _target: &Path, at: &Self::Location) -> Result<(), Self::Location> {
        Err(Error::Unsupported {
            at: at.clone(),
            action: Action::Create,
        })
    }

    fn copy_metadata(&self, from: &Self::Item, to: &Self::Location) -> Result<(), Self::Location>;

    fn can_rename_across(&self, from: &Self::Location, to: &Self::Location) -> bool;

    fn rename_across(
        &self,
        from: &Self::Location,
        to: &Self::Location,
    ) -> Result<(), Self::Location>;
}
