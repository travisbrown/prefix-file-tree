# prefix-file-tree

[![Rust build status](https://img.shields.io/github/actions/workflow/status/travisbrown/prefix-file-tree/ci.yaml?branch=main)](https://github.com/travisbrown/prefix-file-tree/actions)
[![Coverage status](https://img.shields.io/codecov/c/github/travisbrown/prefix-file-tree/main.svg)](https://codecov.io/github/travisbrown/prefix-file-tree)

A small Rust library that provides a way to store many files in a predictable nested directory structure
(see ["Pairtrees for Object Storage"][pairtree] for an explanation of the motivation and general idea).

For example, suppose you want to store a set of files by taking the SHA-1 hash of their contents and using
the Base32 encoding as file names. If you have many millions of files to store, it may be impractical to
keep them in a single directory.
This library gives you an easy way to rewrite a file name like `MFRGGZC7MFRGGZC7MFRGGZC7MVTGO2C7`
as `MFR/GG/MFRGGZC7MFRGGZC7MFRGGZC7MVTGO2C7`, and to iterate over the contents of a directory structure like
this in order (even when the hash order doesn't match the ASCII order, as in the case of Base32).

[pairtree]: https://datatracker.ietf.org/doc/html/draft-kunze-pairtree-01