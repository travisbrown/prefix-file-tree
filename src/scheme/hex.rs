use crate::scheme::{Case, Error, Scheme};
use std::borrow::Cow;
use std::ffi::OsStr;
use std::fmt::Write;

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub struct Hex<const N: usize> {
    pub case: Case,
}

impl<const N: usize> Hex<N> {
    #[must_use]
    pub const fn new(case: Case) -> Self {
        Self { case }
    }
}

impl<const N: usize> Scheme for Hex<N> {
    type Name = [u8; N];
    type NameRef<'a> = [u8; N];

    fn fixed_length() -> Option<usize> {
        Some(N * 2)
    }

    fn name_to_string<'a>(&self, name: Self::NameRef<'a>) -> Cow<'a, str> {
        bytes_to_string(self.case, name).into()
    }

    fn name_from_file_stem(&self, file_stem: &OsStr) -> Result<Self::Name, Error> {
        let as_str = file_stem.to_str().ok_or(Error::NonUtf8)?;

        if as_str.len() == N * 2 {
            let mut name = [0; N];

            for i in 0..N {
                name[i] = u8::from_str_radix(&as_str[i * 2..i * 2 + 2], 16).map_err(|_| {
                    let invalid_byte = first_invalid_byte(self.case, &as_str[i..i + 2]);

                    // This is safe given the contract of `from_str_radix` and our `first_invalid_character`.
                    Error::InvalidByte(
                        invalid_byte.expect("There should be at least one invalid character"),
                    )
                })?;
            }

            Ok(name)
        } else {
            Err(Error::InvalidLength(as_str.len()))
        }
    }
}

#[derive(Clone, Copy, Default, Debug, Eq, PartialEq)]
pub struct AnyLengthHex {
    pub case: Case,
}

impl AnyLengthHex {
    #[must_use]
    pub const fn new(case: Case) -> Self {
        Self { case }
    }
}

impl Scheme for AnyLengthHex {
    type Name = Vec<u8>;
    type NameRef<'a> = &'a [u8];

    fn name_to_string<'a>(&self, name: Self::NameRef<'a>) -> Cow<'a, str> {
        bytes_to_string(self.case, name).into()
    }

    fn name_from_file_stem(&self, file_stem: &OsStr) -> Result<Self::Name, Error> {
        let as_str = file_stem.to_str().ok_or(Error::NonUtf8)?;

        if as_str.len() % 2 == 0 {
            (0..as_str.len())
                .step_by(2)
                .map(|i| {
                    u8::from_str_radix(&as_str[i..i + 2], 16).map_err(|_| {
                        let invalid_byte = first_invalid_byte(self.case, &as_str[i..i + 2]);

                        // This is safe given the contract of `from_str_radix` and our `first_invalid_character`.
                        Error::InvalidByte(
                            invalid_byte.expect("There should be at least one invalid character"),
                        )
                    })
                })
                .collect()
        } else {
            Err(Error::InvalidLength(as_str.len()))
        }
    }
}

fn first_invalid_byte(case: Case, value: &str) -> Option<u8> {
    value
        .as_bytes()
        .iter()
        .find(|c| !is_valid_character_byte(case, **c))
        .copied()
}

const fn is_valid_character_byte(case: Case, c: u8) -> bool {
    c.is_ascii_hexdigit()
        && match case {
            Case::Lower => c.is_ascii_digit() || c.is_ascii_lowercase(),
            Case::Upper => c.is_ascii_digit() || c.is_ascii_uppercase(),
            Case::Any => true,
        }
}

fn bytes_to_string<B: AsRef<[u8]>>(case: Case, bytes: B) -> String {
    let mut result = String::new();

    for byte in bytes.as_ref() {
        if case == Case::Upper {
            write!(result, "{byte:02X}")
        } else {
            // We use lowercase for the `Any` case.
            write!(result, "{byte:02x}")
        }
        // Safe because we're writing to a string.
        .expect("Writing to a string should not fail");
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::{Error, Tree};
    use hex::FromHex;
    use std::io::Write;
    use std::path::Path;

    struct Store {
        tree: Tree<crate::scheme::hex::Hex<16>>,
    }

    impl Store {
        fn new<P: AsRef<Path>>(
            base: P,
            prefix_part_lengths: &[usize],
        ) -> Result<Self, crate::builder::Error> {
            Ok(Self {
                tree: Tree::builder(base)
                    .with_scheme(crate::scheme::hex::Hex::<16>::default())
                    .with_prefix_part_lengths(prefix_part_lengths)
                    .build()?,
            })
        }

        fn save<B: AsRef<[u8]> + Copy>(&self, bytes: B) -> Result<bool, Error> {
            let digest = md5::compute(bytes);

            match self.tree.create_file(digest.0)? {
                Some(mut file) => {
                    file.write_all(bytes.as_ref())?;

                    Ok(true)
                }
                None => Ok(false),
            }
        }
    }

    const MINIMAL_JPG_HEX: &str = "ffd8ffe000104a46494600010100000100010000ffdb004300080606070605080707070909080a0c140d0c0b0b0c1912130f141d1a1f1e1d1a1c1c20242e2720222c231c1c2837292c30313434341f27393d38323c2e333432ffdb0043010909090c0b0c180d0d1832211c21323232323232323232323232323232323232323232323232323232323232323232323232323232ffc00011080001000103011100021101031101ffc4001f00000105010101010101000000000000000102030405060708090a0bffc400b51000020103030204030505040400017d010203000411051221314106135161712232819114a1b1c1d1f0e123f1ffda000c03010002110311003f00ff00ffd9";
    const MINIMAL_PNG_HEX: &str = "89504e470d0a1a0a0000000d4948445200000001000000010802000000907724d90000000a49444154789c6360000002000185d114090000000049454e44ae426082";

    fn minimal_jpg_bytes() -> Vec<u8> {
        hex::decode(MINIMAL_JPG_HEX).unwrap()
    }

    fn minimal_png_bytes() -> Vec<u8> {
        hex::decode(MINIMAL_PNG_HEX).unwrap()
    }

    fn empty_bytes() -> Vec<u8> {
        vec![]
    }

    fn text_bytes() -> Vec<u8> {
        b"foo bar baz".to_vec()
    }

    fn minimal_jpg_digest() -> [u8; 16] {
        FromHex::from_hex("79c09c11a8f92599f3c6d389564dd24d").unwrap()
    }

    fn minimal_png_digest() -> [u8; 16] {
        FromHex::from_hex("ddf93a3305d41f70e19bb8a04ac673a5").unwrap()
    }

    fn empty_digest() -> [u8; 16] {
        FromHex::from_hex("d41d8cd98f00b204e9800998ecf8427e").unwrap()
    }

    fn text_digest() -> [u8; 16] {
        FromHex::from_hex("ab07acbb1e496801937adfa772424bf7").unwrap()
    }

    fn test_hex(
        prefix_part_lengths: Vec<usize>,
    ) -> Result<Vec<crate::Entry<[u8; 16]>>, Box<dyn std::error::Error>> {
        let base = tempfile::tempdir()?;

        let store = Store::new(base.path(), &prefix_part_lengths)?;
        let minimal_jpg_added = store.save(&minimal_jpg_bytes())?;
        let minimal_png_added = store.save(&minimal_png_bytes())?;
        let empty_added = store.save(&empty_bytes())?;
        let text_added = store.save(&text_bytes())?;

        assert!(minimal_jpg_added);
        assert!(minimal_png_added);
        assert!(empty_added);
        assert!(text_added);

        let repeat_minimal_jpg_added = store.save(&minimal_jpg_bytes())?;
        let repeat_minimal_png_added = store.save(&minimal_png_bytes())?;
        let repeat_empty_added = store.save(&empty_bytes())?;
        let repeat_text_added = store.save(&text_bytes())?;

        assert!(!repeat_minimal_jpg_added);
        assert!(!repeat_minimal_png_added);
        assert!(!repeat_empty_added);
        assert!(!repeat_text_added);

        let inferred_prefix_parts_length = Tree::infer_prefix_part_lengths(base.path())?;

        assert_eq!(inferred_prefix_parts_length, Some(prefix_part_lengths));

        let entries = store.tree.entries().collect::<Result<Vec<_>, _>>()?;
        let digests = entries.iter().map(|entry| entry.name).collect::<Vec<_>>();

        let expected_digests = vec![
            minimal_jpg_digest(),
            text_digest(),
            empty_digest(),
            minimal_png_digest(),
        ];

        assert_eq!(entries.len(), 4);
        assert_eq!(digests, expected_digests);

        Ok(entries)
    }

    #[test]
    fn test_hex_empty() -> Result<(), Box<dyn std::error::Error>> {
        test_hex(vec![])?;

        Ok(())
    }

    #[test]
    fn test_hex_1() -> Result<(), Box<dyn std::error::Error>> {
        test_hex(vec![1])?;

        Ok(())
    }

    #[test]
    fn test_hex_2_2() -> Result<(), Box<dyn std::error::Error>> {
        test_hex(vec![2, 2])?;

        Ok(())
    }

    #[test]
    fn test_hex_16_3() -> Result<(), Box<dyn std::error::Error>> {
        test_hex(vec![16, 3])?;

        Ok(())
    }

    #[test]
    fn test_hex_19_13() -> Result<(), Box<dyn std::error::Error>> {
        test_hex(vec![19, 13])?;

        Ok(())
    }
}
