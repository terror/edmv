## edmv 📦

**edmv** is a tool that lets you bulk rename files fast using your preferred
text editor.

### Installation

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

### Usage

```
Usage: edmv [OPTIONS] [sources]...

Arguments:
  [sources]...  Paths to edit

Options:
      --editor <EDITOR>  Editor command to use
      --force            Overwrite existing files
      --temp             Rename sources to temporary files internally
      --dry-run          Run without making any changes
  -h, --help             Print help
```

### Prior Art

**edmv** is a tested and improved re-implementation of [Casey's](https://github.com/casey)
implementation in [Python](https://github.com/casey/edmv), do check it out!
