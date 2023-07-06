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
Usage: edmv [OPTIONS] [paths]...

Arguments:
  [paths]...  Paths to edit

Options:
      --dry-run          Run without making any changes
      --editor <EDITOR>  Editor command to use
      --force            Overwrite existing files
  -h, --help             Print help
```

### Prior Art

**edmv** is a tested and improved re-implementation of [Casey's](https://github.com/casey)
implementation in [Python](https://github.com/casey/edmv), do check it out!
