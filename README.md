## edmv 📦

[![release](https://img.shields.io/github/release/terror/edmv.svg?label=release&style=flat&labelColor=282c34&logo=github)](https://github.com/terror/edmv/releases/latest)
[![CI](https://github.com/terror/edmv/actions/workflows/ci.yaml/badge.svg)](https://github.com/terror/edmv/actions/workflows/ci.yaml)
[![codecov](https://codecov.io/gh/terror/edmv/graph/badge.svg?token=7CH4XDXO7Z)](https://codecov.io/gh/terror/edmv)
[![crates.io](https://shields.io/crates/v/edmv.svg)](https://crates.io/crates/edmv)
[![downloads](https://img.shields.io/crates/d/edmv)](https://crates.io/crates/edmv)
[![dependency status](https://deps.rs/repo/github/terror/edmv/status.svg)](https://deps.rs/repo/github/terror/edmv)

**edmv** is a tool that lets you bulk rename files fast using your preferred
text editor.

## Demo

Below is a short demo showcasing the main functionality of the program:

[![asciicast](https://asciinema.org/a/33OVZX9m1PZcyqYvdqmtvBRRv.svg)](https://asciinema.org/a/33OVZX9m1PZcyqYvdqmtvBRRv)

## Installation

You can install the **edmv** command-line utility via the rust package manager
[cargo](https://doc.rust-lang.org/cargo/):

```bash
cargo install edmv
```

...or you can build it from source:

```bash
git clone https://github.com/terror/edmv
cd edmv
cargo install --path .
```

...or you can download one of the pre-built binaries from the
[releases](https://github.com/terror/edmv/releases) page.


## Usage

Below is the output of `edmv --help`:

```
Bulk rename files using your favorite editor

Usage: edmv [OPTIONS] [sources]...

Arguments:
  [sources]...  Paths to edit

Options:
      --editor <EDITOR>  Editor command to use
      --force            Overwrite existing files
      --resolve          Resolve conflicting renames
      --dry-run          Run without making any changes
  -h, --help             Print help
  -V, --version          Print version
```

An option of note is the `--resolve` option, this applies to sources an
intermediate rename to either a temporary directory or file - automatically
handling conflicts such as overlapping or circular renames.

## Prior Art

**edmv** is a tested and extended re-implementation of
the version [Casey](https://github.com/casey) wrote in
[Python](https://github.com/casey/edmv), do check it out!
