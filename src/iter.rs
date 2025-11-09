use crate::{Entry, scheme::Scheme};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error")]
    Io(#[from] std::io::Error),
    #[error("Invalid prefix part path")]
    InvalidPrefixPart(PathBuf),
    #[error("Invalid file stem")]
    InvalidFileStem(PathBuf),
    #[error("Expected file")]
    ExpectedFile(PathBuf),
    #[error("Expected directory")]
    ExpectedDirectory(PathBuf),
    #[error("Invalid extension")]
    InvalidExtension(Option<OsString>),
    #[error("Invalid file stem length")]
    InvalidFileStemLength(Option<usize>),
    #[error("Scheme parse error")]
    Scheme(#[from] crate::scheme::Error),
}

pub struct Entries<'a, S> {
    stack: Vec<Vec<PathBuf>>,
    level: Option<usize>,
    tree: &'a crate::Tree<S>,
}

impl<'a, S> Entries<'a, S> {
    pub(crate) fn new(tree: &'a crate::Tree<S>) -> Self {
        Self {
            stack: vec![vec![tree.base.clone()]],
            level: None,
            tree,
        }
    }

    fn is_last(&self) -> bool {
        self.level == Some(self.tree.prefix_part_lengths.len())
    }

    fn current_prefix_part_length(&self) -> Option<usize> {
        self.level
            .and_then(|level| self.tree.prefix_part_lengths.get(level))
            .copied()
    }

    fn increment_level(&mut self) {
        self.level = Some(self.level.take().map_or(0, |level| level + 1));
    }

    const fn decrement_level(&mut self) {
        if let Some(level) = self.level.take()
            && level != 0
        {
            self.level = Some(level - 1);
        }
    }

    fn validate_extension<P: AsRef<Path>>(&self, path: P) -> Result<(), Option<OsString>> {
        match &self.tree.extension_constraint {
            None => Ok(()),
            Some(crate::constraint::Extension::None) => path
                .as_ref()
                .extension()
                .map_or(Ok(()), |extension| Err(Some(extension.to_os_string()))),
            Some(crate::constraint::Extension::Any) => {
                path.as_ref().extension().map_or(Err(None), |_| Ok(()))
            }
            Some(crate::constraint::Extension::Fixed(expected_extension)) => {
                path.as_ref().extension().map_or(Err(None), |extension| {
                    if **expected_extension == *extension {
                        Ok(())
                    } else {
                        Err(Some(extension.to_os_string()))
                    }
                })
            }
        }
    }

    fn validate_file_stem_length<P: AsRef<Path>>(&self, path: P) -> Result<(), Option<usize>> {
        match &self.tree.length_constraint {
            None => Ok(()),
            Some(crate::constraint::Length::Fixed(length)) => {
                path.as_ref().file_stem().map_or(Err(None), |file_stem| {
                    if file_stem.len() == *length {
                        Ok(())
                    } else {
                        Err(Some(file_stem.len()))
                    }
                })
            }
            Some(crate::constraint::Length::Range(minimum, maximum)) => {
                path.as_ref().file_stem().map_or(Err(None), |file_stem| {
                    if file_stem.len() >= *minimum && file_stem.len() < *maximum {
                        Ok(())
                    } else {
                        Err(Some(file_stem.len()))
                    }
                })
            }
        }
    }
}

impl<S: Scheme> Iterator for Entries<'_, S> {
    type Item = Result<Entry<S::Name>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.stack.pop().and_then(|mut next_paths| {
            if let Some(next_path) = next_paths.pop() {
                if self.is_last() {
                    self.stack.push(next_paths);

                    Some(self.path_to_entry(next_path))
                } else {
                    self.increment_level();

                    self.path_to_paths(next_path, self.current_prefix_part_length())
                        .map_or_else(
                            |error| Some(Err(error)),
                            |next_level| {
                                self.stack.push(next_paths);
                                self.stack.push(next_level);

                                self.next()
                            },
                        )
                }
            } else {
                self.decrement_level();

                self.next()
            }
        })
    }
}

impl<S: Scheme> Entries<'_, S> {
    fn path_to_entry(&self, path: PathBuf) -> Result<Entry<S::Name>, Error> {
        if path.is_file() {
            self.validate_extension(&path)
                .map_err(Error::InvalidExtension)?;

            self.validate_file_stem_length(&path)
                .map_err(Error::InvalidFileStemLength)?;

            let file_stem = path
                .file_stem()
                .ok_or_else(|| Error::InvalidFileStem(path.clone()))?;

            let name = self.tree.scheme.name_from_file_stem(file_stem)?;

            Ok(Entry { name, path })
        } else {
            Err(Error::ExpectedFile(path))
        }
    }
    fn path_to_paths(
        &self,
        path: PathBuf,
        prefix_part_length: Option<usize>,
    ) -> Result<Vec<PathBuf>, Error> {
        if path.is_dir() {
            let mut paths = std::fs::read_dir(path)?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<PathBuf>, std::io::Error>>()
                .map_err(Error::from)?;

            // If our ordering for prefix parts fails, we simply leave them in the original order.
            //
            // The error should be caught by later validation.
            paths.sort_by(|a, b| {
                let directory_name_a = a.file_name();
                let directory_name_b = b.file_name();

                directory_name_a
                    .zip(directory_name_b)
                    .and_then(|(directory_name_a, directory_name_b)| {
                        self.tree
                            .scheme
                            .cmp_prefix_part(directory_name_a, directory_name_b)
                            .ok()
                    })
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .reverse()
            });

            match prefix_part_length {
                Some(prefix_part_length) => {
                    let invalid_path = paths.iter().find(|path| {
                        path.file_name()
                            .is_none_or(|directory_name| directory_name.len() != prefix_part_length)
                    });

                    // Clippy is wrong here, since `map_or` would require us to clone `paths`.
                    #[allow(clippy::option_if_let_else)]
                    match invalid_path {
                        Some(invalid_path) => Err(Error::InvalidPrefixPart(invalid_path.clone())),
                        None => Ok(paths),
                    }
                }
                None => Ok(paths),
            }
        } else {
            Err(Error::ExpectedDirectory(path))
        }
    }
}
