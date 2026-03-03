use std::ops::Range;
use std::path::PathBuf;

use crate::{constraint, scheme};

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("Inconsistent prefix part lengths")]
    InconsistentPrefixPartLengths {
        prefix_part_lengths_total: usize,
        length_constraint: constraint::Length,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeBuilder<S> {
    base: PathBuf,
    length_constraint: Option<crate::constraint::Length>,
    extension_constraint: Option<crate::constraint::Extension>,
    prefix_part_lengths: Option<Vec<usize>>,
    scheme: S,
}

impl TreeBuilder<crate::scheme::Identity> {
    pub(crate) const fn new(base: PathBuf) -> Self {
        Self {
            base,
            length_constraint: None,
            extension_constraint: None,
            prefix_part_lengths: None,
            scheme: scheme::Identity,
        }
    }
}

impl<S> TreeBuilder<S> {
    pub fn build(self) -> Result<crate::Tree<S>, Error> {
        let tree = self.into_tree();

        match tree.length_constraint {
            Some(length_constraint) => {
                let prefix_part_lengths_total =
                    tree.prefix_part_lengths.iter().copied().sum::<usize>();

                // For a range, the prefix total must fit within the minimum valid name length.
                let threshold = match length_constraint {
                    constraint::Length::Fixed(length) => length,
                    constraint::Length::Range(minimum, _) => minimum,
                };

                if prefix_part_lengths_total <= threshold {
                    Ok(tree)
                } else {
                    Err(Error::InconsistentPrefixPartLengths {
                        prefix_part_lengths_total,
                        length_constraint,
                    })
                }
            }
            None => Ok(tree),
        }
    }

    /// Internal unvalidated conversion.
    fn into_tree(self) -> crate::Tree<S> {
        crate::Tree {
            base: self.base,
            length_constraint: self.length_constraint,
            extension_constraint: self.extension_constraint,
            prefix_part_lengths: self.prefix_part_lengths.unwrap_or_default(),
            scheme: self.scheme,
        }
    }

    #[must_use]
    pub fn with_no_extension(self) -> Self {
        Self {
            extension_constraint: Some(crate::constraint::Extension::None),
            ..self
        }
    }

    #[must_use]
    pub fn with_extension<E: Into<String>>(self, extension: E) -> Self {
        Self {
            extension_constraint: Some(crate::constraint::Extension::Fixed(extension.into())),
            ..self
        }
    }

    #[must_use]
    pub fn with_any_extension(self) -> Self {
        Self {
            extension_constraint: Some(crate::constraint::Extension::Any),
            ..self
        }
    }

    #[must_use]
    pub fn with_length(self, length: usize) -> Self {
        Self {
            length_constraint: Some(length.into()),
            ..self
        }
    }

    #[must_use]
    pub fn with_length_range(self, range: Range<usize>) -> Self {
        Self {
            length_constraint: Some(range.into()),
            ..self
        }
    }

    #[must_use]
    pub fn with_prefix_part_lengths<T: AsRef<[usize]>>(self, prefix_part_lengths: T) -> Self {
        Self {
            prefix_part_lengths: Some(prefix_part_lengths.as_ref().to_vec()),
            ..self
        }
    }

    #[must_use]
    pub fn with_scheme<T: crate::scheme::Scheme>(self, scheme: T) -> TreeBuilder<T> {
        let length_constraint = T::fixed_length().map_or(self.length_constraint, |fixed_length| {
            Some(fixed_length.into())
        });

        TreeBuilder {
            base: self.base,
            length_constraint,
            extension_constraint: self.extension_constraint,
            prefix_part_lengths: self.prefix_part_lengths,
            scheme,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Tree, scheme};

    #[test]
    fn test_range_constraint_rejects_when_prefix_total_exceeds_minimum() {
        let base = tempfile::tempdir().unwrap();
        let result = Tree::builder(base.path())
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([3])
            .with_length_range(2..10)
            .build();

        assert_eq!(
            result,
            Err(Error::InconsistentPrefixPartLengths {
                prefix_part_lengths_total: 3,
                length_constraint: constraint::Length::Range(2, 10),
            })
        );
    }

    #[test]
    fn test_range_constraint_accepts_when_prefix_total_equals_minimum() {
        let base = tempfile::tempdir().unwrap();
        let result = Tree::builder(base.path())
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([2])
            .with_length_range(2..10)
            .build();

        assert!(result.is_ok());
    }

    #[test]
    fn test_fixed_constraint_still_rejects_when_prefix_total_exceeds_length() {
        let base = tempfile::tempdir().unwrap();
        let result = Tree::builder(base.path())
            .with_scheme(scheme::Utf8)
            .with_prefix_part_lengths([3])
            .with_length(2)
            .build();

        assert_eq!(
            result,
            Err(Error::InconsistentPrefixPartLengths {
                prefix_part_lengths_total: 3,
                length_constraint: constraint::Length::Fixed(2),
            })
        );
    }
}
