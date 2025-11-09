use std::borrow::Cow;
use std::cmp::Ordering;
use std::ffi::{OsStr, OsString};

#[cfg(feature = "data-encoding")]
pub mod encoding;
pub mod hex;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("Expected UTF-8")]
    NonUtf8,
    #[error("Invalid byte")]
    InvalidByte(u8),
    #[error("Invalid length")]
    InvalidLength(usize),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Case {
    #[default]
    Lower,
    Upper,
    Any,
}

pub trait Scheme {
    type Name;

    #[must_use]
    fn fixed_length() -> Option<usize> {
        None
    }

    fn name_to_string<'a>(&self, name: &'a Self::Name) -> Cow<'a, str>;
    fn name_from_file_stem(&self, file_stem: &OsStr) -> Result<Self::Name, Error>;

    fn cmp_prefix_part(&self, a: &OsStr, b: &OsStr) -> Result<Ordering, Error> {
        Ok(a.cmp(b))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Identity;

impl Scheme for Identity {
    type Name = OsString;

    fn name_to_string<'a>(&self, name: &'a Self::Name) -> Cow<'a, str> {
        name.as_os_str().to_string_lossy()
    }

    fn name_from_file_stem(&self, file_stem: &OsStr) -> Result<Self::Name, Error> {
        Ok(file_stem.to_os_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Utf8;

impl Scheme for Utf8 {
    type Name = String;

    fn name_to_string<'a>(&self, name: &'a Self::Name) -> Cow<'a, str> {
        name.into()
    }

    fn name_from_file_stem(&self, file_stem: &OsStr) -> Result<Self::Name, Error> {
        file_stem
            .to_str()
            .map(std::string::ToString::to_string)
            .ok_or(Error::NonUtf8)
    }
}
