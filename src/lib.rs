#![warn(clippy::all, clippy::pedantic, clippy::nursery, rust_2018_idioms)]
#![allow(clippy::missing_errors_doc)]
#![forbid(unsafe_code)]
use std::fs::File;
use std::path::{Path, PathBuf};

pub mod builder;
pub mod constraint;
pub mod iter;
pub mod scheme;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("Expected file")]
    ExpectedFile(PathBuf),
    #[error("Expected directory")]
    ExpectedDirectory(PathBuf),
    #[error("Invalid directory")]
    InvalidDirectory(PathBuf),
    #[error("Invalid name")]
    InvalidName(String),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Entry<N> {
    pub name: N,
    pub path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tree<S> {
    base: PathBuf,
    length_constraint: Option<constraint::Length>,
    extension_constraint: Option<constraint::Extension>,
    prefix_part_lengths: Vec<usize>,
    scheme: S,
}

impl<S: scheme::Scheme> Tree<S> {
    /// Return the path through the tree for the given name.
    ///
    /// Note that this function ignores any configured extension constraint, or any extension at
    /// for a file with this file stem at the specified directory.
    fn name_path(&self, name: &S::Name) -> Result<PathBuf, String> {
        let name_string = self.scheme.name_to_string(name);

        if name_string.len() >= self.prefix_part_lengths_total().max(1) {
            let mut name_remaining = name_string.as_ref();
            let mut path = self.base.clone();

            for prefix_part_length in &self.prefix_part_lengths {
                let next = &name_remaining[0..*prefix_part_length];
                name_remaining = &name_remaining[*prefix_part_length..];

                path.push(next);
            }

            path.push(name_string.as_ref());

            Ok(path)
        } else {
            Err(name_string.to_string())
        }
    }

    /// Return the path through the tree for the given name, including any fixed extension.
    pub fn path(&self, name: &S::Name) -> Result<PathBuf, String> {
        let mut name_path = self.name_path(name)?;

        if let Some(constraint::Extension::Fixed(extension)) = &self.extension_constraint {
            name_path.add_extension(extension);
        }

        Ok(name_path)
    }

    fn prefix_part_lengths_total(&self) -> usize {
        self.prefix_part_lengths.iter().sum()
    }

    /// Try to open a file for reading for the given name, including any fixed extension.
    ///
    /// Note that this function will probably not do the right thing for any extension
    /// configuration that does not either prohibit extensions or require a fixed extension.
    pub fn open_file(&self, name: &S::Name) -> Result<Option<File>, Error> {
        let path = self.path(name).map_err(Error::InvalidName)?;

        match File::open(&path) {
            Ok(file) => {
                if path.is_file() {
                    Ok(Some(file))
                } else {
                    Err(Error::ExpectedFile(path))
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Try to create a file for writing for the given name, including any fixed extension.
    ///
    /// Note that this function will probably not do the right thing for any extension
    /// configuration that does not either prohibit extensions or require a fixed extension.
    pub fn create_file(&self, name: &S::Name) -> Result<Option<File>, Error> {
        let path = self.path(name).map_err(Error::InvalidName)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        match File::create_new(path) {
            Ok(file) => {
                file.lock()?;

                Ok(Some(file))
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
            Err(other) => Err(other.into()),
        }
    }

    #[must_use]
    pub fn entries(&self) -> iter::Entries<'_, S> {
        iter::Entries::new(self)
    }
}

impl Tree<scheme::Identity> {
    pub fn builder<P: AsRef<Path>>(base: P) -> builder::TreeBuilder<scheme::Identity> {
        builder::TreeBuilder::new(base.as_ref().to_path_buf())
    }

    /// Infer the prefix part lengths used to create a store.
    ///
    /// The result will be empty if and only if the store has no files (even if there are directories).
    ///
    /// If this function returns a result, it is guaranteed to be correct if the store is valid, but the validity is not checked.
    pub fn infer_prefix_part_lengths<P: AsRef<Path>>(base: P) -> Result<Option<Vec<usize>>, Error> {
        if base.as_ref().is_dir() {
            let first = std::fs::read_dir(base)?
                .next()
                .map_or(Ok(None), |entry| entry.map(|entry| Some(entry.path())))?;

            let mut acc = vec![];

            let is_empty = first
                .map(|first| Self::infer_prefix_part_lengths_rec(&first, &mut acc))
                .map_or(Ok(true), |value| value)?;

            Ok(if is_empty { None } else { Some(acc) })
        } else {
            Err(Error::ExpectedDirectory(base.as_ref().to_path_buf()))
        }
    }

    // Return value indicates whether the store has no files.
    fn infer_prefix_part_lengths_rec<P: AsRef<Path>>(
        current: P,
        acc: &mut Vec<usize>,
    ) -> Result<bool, Error> {
        if current.as_ref().is_file() {
            Ok(false)
        } else {
            let directory_name = current
                .as_ref()
                .file_name()
                .ok_or_else(|| Error::InvalidDirectory(current.as_ref().to_path_buf()))?;

            acc.push(directory_name.len());

            let next = std::fs::read_dir(current)?
                .next()
                .map_or(Ok(None), |entry| entry.map(|entry| Some(entry.path())))?;

            next.map_or(Ok(true), |next| {
                Self::infer_prefix_part_lengths_rec(next, acc)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_path_with_valid_prefix_lengths() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let tree = Tree::builder(temp_dir.path())
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([2, 2])
            .build()?;

        let path = tree.path(&"abcdef".to_string())?;
        assert!(path.to_string_lossy().ends_with("/ab/cd/abcdef"));

        Ok(())
    }

    #[test]
    fn test_path_boundary_case_equal_length() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let tree = Tree::builder(temp_dir.path())
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([2, 1])
            .build()?;

        // Name length (3) equals prefix total (3).
        let path = tree.path(&"abc".to_string())?;
        assert!(path.to_string_lossy().ends_with("/ab/c/abc"));

        Ok(())
    }

    #[test]
    fn test_path_too_short_name() {
        let temp_dir = tempfile::tempdir().unwrap();
        let tree = Tree::builder(temp_dir.path())
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([2, 2])
            .build()
            .unwrap();

        // Name length (3) is less than prefix total (4).
        let result = tree.path(&"abc".to_string());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "abc");
    }

    #[test]
    fn test_open_file_nonexistent() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let tree = Tree::builder(temp_dir.path())
            .with_scheme(scheme::Utf8)
            .build()?;

        let result = tree.open_file(&"nonexistent".to_string())?;
        assert!(
            result.is_none(),
            "Should return `Ok(None)` for nonexistent file"
        );

        Ok(())
    }

    #[test]
    fn test_open_file_exists() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let tree = Tree::builder(temp_dir.path())
            .with_scheme(scheme::Utf8)
            .build()?;

        // Create a file.
        let test_name = "testfile".to_string();
        let mut file = tree
            .create_file(&test_name)?
            .expect("Failed to create file");
        file.write_all(b"test content")?;
        drop(file);

        // Try to open it.
        let opened = tree.open_file(&test_name)?;
        assert!(
            opened.is_some(),
            "Should return `Ok(Some(file))` for existing file"
        );

        Ok(())
    }

    #[test]
    fn test_open_file_directory_instead_of_file() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let tree = Tree::builder(temp_dir.path())
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([2])
            .build()?;

        // Create a file, which will create directory `ab`.
        let mut file = tree
            .create_file(&"abcd".to_string())?
            .expect("Failed to create");
        file.write_all(b"test")?;
        drop(file);

        let dir_name = "zz".to_string();
        let dir_path = tree.path(&dir_name)?;
        std::fs::create_dir_all(&dir_path)?;

        // Now try to open `zz` which exists as a directory.
        let result = tree.open_file(&dir_name);

        match result {
            Err(Error::ExpectedFile(_)) | Ok(None) => {
                // Expected behavior (detected it's a directory or couldn't open it)
            }
            Ok(Some(_)) => {
                panic!("Should not return `Ok(Some)` for a directory")
            }
            other => {
                panic!("Unexpected result: {other:?}")
            }
        }

        Ok(())
    }

    #[test]
    fn test_open_file_symlink_to_directory() -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let temp_dir = tempfile::tempdir()?;
            let tree = Tree::builder(temp_dir.path())
                .with_scheme(scheme::Utf8)
                .build()?;

            // Create a directory and a symlink to it.
            let dir_path = temp_dir.path().join("somedir");
            std::fs::create_dir(&dir_path)?;

            let link_name = "symlink".to_string();
            let link_path = temp_dir.path().join(&link_name);
            symlink(&dir_path, &link_path)?;

            // Try to open the symlink (which points to a directory).
            let result = tree.open_file(&link_name);

            match result {
                Err(Error::ExpectedFile(_)) | Ok(None) => {
                    // Should return Err(ExpectedFile) because target is a directory.
                }
                other => panic!("Expected Err or Ok(None), got {other:?}"),
            }
        }

        Ok(())
    }

    #[test]
    fn test_create_file_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let tree = Tree::builder(temp_dir.path())
            .with_scheme(scheme::Utf8)
            .build()?;

        let name = "testfile".to_string();

        // First creation should succeed.
        let first = tree.create_file(&name)?;
        assert!(first.is_some(), "First creation should return Some(file)");
        drop(first);

        // Second creation should return `None` (file exists).
        let second = tree.create_file(&name)?;
        assert!(second.is_none(), "Second creation should return None");

        Ok(())
    }

    #[test]
    fn test_create_file_with_nested_dirs() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let tree = Tree::builder(temp_dir.path())
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([2, 2, 2])
            .build()?;

        let name = "abcdefgh".to_string();
        let mut file = tree.create_file(&name)?.expect("Failed to create file");
        file.write_all(b"nested")?;
        drop(file);

        // Verify the directory structure was created.
        let path = tree.path(&name)?;
        assert!(path.exists());
        assert!(path.is_file());
        assert!(path.to_string_lossy().contains("/ab/cd/ef/"));

        Ok(())
    }

    #[test]
    fn test_infer_empty_directory() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;

        let result = Tree::infer_prefix_part_lengths(temp_dir.path())?;
        assert_eq!(result, None, "Empty directory should return None");

        Ok(())
    }

    #[test]
    fn test_infer_with_files_no_subdirs() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;

        // Create a file directly in the base directory.
        std::fs::File::create(temp_dir.path().join("file.txt"))?;

        let result = Tree::infer_prefix_part_lengths(temp_dir.path())?;
        assert_eq!(
            result,
            Some(vec![]),
            "File in root should give empty prefix list"
        );

        Ok(())
    }

    #[test]
    fn test_infer_with_nested_structure() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;

        // Create structure: `ab/cd/file.txt`.
        let dir1 = temp_dir.path().join("ab");
        let dir2 = dir1.join("cd");
        std::fs::create_dir_all(&dir2)?;
        std::fs::File::create(dir2.join("file.txt"))?;

        let result = Tree::infer_prefix_part_lengths(temp_dir.path())?;
        assert_eq!(
            result,
            Some(vec![2, 2]),
            "Should infer `[2, 2]` from `ab/cd/`"
        );

        Ok(())
    }

    #[test]
    fn test_infer_on_file_instead_of_directory() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();

        let result = Tree::infer_prefix_part_lengths(temp_file.path());
        assert!(
            result.is_err(),
            "Should return error when given a file path"
        );
        match result {
            Err(Error::ExpectedDirectory(_)) => (),
            other => panic!("Expected `Err(ExpectedDirectory)`, got {other:?}"),
        }
    }

    #[test]
    fn test_path_with_empty_prefix_parts() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let tree = Tree::builder(temp_dir.path())
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([])
            .build()?;

        let path = tree.path(&"filename".to_string())?;
        assert!(path.to_string_lossy().ends_with("/filename"));
        assert!(!path.to_string_lossy().contains("//"));

        Ok(())
    }

    #[test]
    fn test_entries_iteration() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let tree = Tree::builder(temp_dir.path())
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([1])
            .build()?;

        // Create some files.
        let names = vec!["aaa", "abc", "bcd", "bbb"];
        for name in &names {
            let mut file = tree
                .create_file(&(*name).to_string())?
                .expect("create failed");
            file.write_all(name.as_bytes())?;
            drop(file);
        }

        // Collect all entries.
        let entries: Vec<_> = tree.entries().collect::<Result<Vec<_>, _>>()?;
        assert_eq!(entries.len(), 4, "Should find all four files");

        // Check that we got the files (order depends on scheme sorting).
        let entry_names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
        for name in &names {
            assert!(
                entry_names.contains(&(*name).to_string()),
                "Should contain {name}"
            );
        }

        Ok(())
    }

    #[test]
    fn test_example_with_fixed_extension() -> Result<(), Box<dyn std::error::Error>> {
        let tree = Tree::builder("examples/extensions/fixed-01/")
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([2, 2, 2])
            .with_length(8)
            .with_extension("txt")
            .build()?;

        let file = tree.open_file(&"01234567".to_string())?;
        assert!(file.is_some());

        let file = tree.open_file(&"98765432".to_string())?;
        assert!(file.is_some());

        let entries: Vec<_> = tree.entries().collect::<Result<Vec<_>, _>>()?;
        assert_eq!(entries.len(), 2, "Should find both files");

        Ok(())
    }

    #[test]
    fn test_example_with_mixed_extensions_and_no_constraint()
    -> Result<(), Box<dyn std::error::Error>> {
        let tree = Tree::builder("examples/extensions/mixed-01/")
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([2, 2, 2])
            .with_length(8)
            .build()?;

        let entries: Vec<_> = tree.entries().collect::<Result<Vec<_>, _>>()?;
        assert_eq!(entries.len(), 2, "Should find both files");

        Ok(())
    }

    #[test]
    fn test_example_with_mixed_extensions_and_any_constraint_fails()
    -> Result<(), Box<dyn std::error::Error>> {
        let tree = Tree::builder("examples/extensions/mixed-01/")
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([2, 2, 2])
            .with_length(8)
            .with_any_extension()
            .build()?;

        let entries = tree.entries().collect::<Result<Vec<_>, _>>();

        match entries {
            Err(super::iter::Error::InvalidExtension(None)) => {}
            Err(error) => {
                panic!("Unexpected error: {error:?}");
            }
            Ok(_) => {
                panic!("Expected error on missing extension");
            }
        }

        Ok(())
    }

    #[test]
    fn test_example_with_mixed_extensions_and_fixed_constraint_fails()
    -> Result<(), Box<dyn std::error::Error>> {
        let tree = Tree::builder("examples/extensions/mixed-01/")
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([2, 2, 2])
            .with_length(8)
            .with_extension("txt")
            .build()?;

        let file = tree.open_file(&"01234567".to_string())?;
        assert!(file.is_some());

        let file = tree.open_file(&"98765432".to_string())?;
        assert!(file.is_none());

        let entries = tree.entries().collect::<Result<Vec<_>, _>>();

        match entries {
            Err(super::iter::Error::InvalidExtension(None)) => {}
            Err(error) => {
                panic!("Unexpected error: {error:?}");
            }
            Ok(_) => {
                panic!("Expected error on missing extension");
            }
        }

        Ok(())
    }
}
