use crate::scheme::{Case, Error, Scheme};
use data_encoding::BASE32;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::ffi::OsStr;

/// Fixed-length Base32 name encoding scheme.
///
/// Note that padding is not handled, and that `N` must be a multiple of 5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Base32<const N: usize> {
    pub case: Case,
}

impl<const N: usize> Base32<N> {
    const VALID: () = assert!(N.is_multiple_of(5), "N must be a multiple of 5 for Base32 encoding");

    #[must_use]
    pub const fn new(case: Case) -> Self {
        let () = Self::VALID;
        Self { case }
    }
}

impl<const N: usize> Scheme for Base32<N> {
    type Name = [u8; N];
    type NameRef<'a> = [u8; N];

    fn fixed_length() -> Option<usize> {
        Some(N / 5 * 8)
    }

    fn name_to_string<'a>(&self, name: Self::NameRef<'a>) -> Cow<'a, str> {
        BASE32.encode(&name).into()
    }

    fn cmp_prefix_part(&self, a: &OsStr, b: &OsStr) -> Result<Ordering, Error> {
        let a_chars = a
            .as_encoded_bytes()
            .iter()
            .map(|byte| Base32Char::try_from(*byte))
            .collect::<Result<Vec<_>, _>>()?;

        let b_chars = b
            .as_encoded_bytes()
            .iter()
            .map(|byte| Base32Char::try_from(*byte))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(a_chars.cmp(&b_chars))
    }

    fn name_from_file_stem(&self, file_stem: &OsStr) -> Result<Self::Name, Error> {
        let () = Self::VALID;
        let as_bytes = file_stem.as_encoded_bytes();

        if as_bytes.len() == N / 5 * 8 {
            let decoded = BASE32
                .decode(as_bytes)
                .map_err(|error| Error::InvalidByte(as_bytes[error.position]))?;

            Ok(decoded.try_into().expect("Invalid decoded bytes length"))
        } else {
            Err(Error::InvalidLength(as_bytes.len()))
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Base32Char {
    Alphabetic(u8),
    Numeric(u8),
}

impl TryFrom<u8> for Base32Char {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value.is_ascii_uppercase() {
            Ok(Self::Alphabetic(value))
        } else if (b'2'..=b'7').contains(&value) {
            Ok(Self::Numeric(value))
        } else {
            Err(Error::InvalidByte(value))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Tree;
    use std::io::Write;

    #[test]
    fn test_base32() -> Result<(), Box<dyn std::error::Error>> {
        let base = tempfile::tempdir()?;
        let prefix_part_lengths = vec![3, 2];

        let name_1 = b"abcd_abcd_abcd_abcd_";
        //let name_2 = b"abcd_abcd_abcd_efgh_";
        let name_2 = &[255u8; 20];
        let name_3 = b"abcd_abcd_abcd_efgh_";

        let tree = Tree::builder(base)
            .with_scheme(crate::scheme::encoding::Base32::<20>::new(
                crate::scheme::Case::Lower,
            ))
            .with_prefix_part_lengths(prefix_part_lengths)
            .build()?;

        let mut file = tree.create_file(*name_1)?.expect("Unexpected file");

        file.write_all(b"foo")?;

        let file = tree.create_file(*name_1)?;

        assert!(file.is_none());

        let mut file = tree.create_file(*name_2)?.expect("Unexpected file");

        file.write_all(b"bar")?;

        let mut file = tree.create_file(*name_3)?.expect("Unexpected file");

        file.write_all(b"qux")?;

        let entries = tree.entries().collect::<Result<Vec<_>, _>>()?;

        assert!(
            entries[0]
                .path
                .to_string_lossy()
                .ends_with("/MFR/GG/MFRGGZC7MFRGGZC7MFRGGZC7MFRGGZC7")
        );

        assert_eq!(
            entries
                .into_iter()
                .map(|entry| entry.name)
                .collect::<Vec<_>>(),
            vec![*name_1, *name_3, *name_2]
        );

        Ok(())
    }
}
