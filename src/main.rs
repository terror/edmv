use {
  anyhow::bail,
  clap::Parser,
  path_absolutize::*,
  std::{
    collections::HashMap,
    env, fs,
    io::Write,
    path::Path,
    process::{self, Command},
  },
  tempfile::{Builder, NamedTempFile},
};

trait TempfileExt {
  fn write(self, content: &str) -> Result<Self>
  where
    Self: Sized;
}

impl TempfileExt for NamedTempFile {
  fn write(mut self, content: &str) -> Result<Self> {
    writeln!(self, "{}", content)?;
    Ok(self)
  }
}

#[derive(Debug, Parser)]
struct Arguments {
  #[clap(long, help = "Run without making any changes")]
  dry_run: bool,
  #[clap(long, help = "Editor command to use")]
  editor: Option<String>,
  #[clap(long, help = "Overwrite existing files")]
  force: bool,
  #[clap(name = "paths", help = "Paths to edit")]
  paths: Vec<String>,
}

impl Arguments {
  fn run(self) -> Result {
    let editor = self
      .editor
      .unwrap_or(env::var("EDITOR").unwrap_or("vi".to_string()));

    let absent = self
      .paths
      .clone()
      .into_iter()
      .filter(|path| fs::metadata(path).is_err())
      .collect::<Vec<String>>();

    if !absent.is_empty() {
      bail!("Found path(s) that do not exist: {}", absent.join(", "));
    }

    let file = Builder::new()
      .prefix("edmv-")
      .suffix(".txt")
      .tempfile()?
      .write(&self.paths.join("\n"))?;

    let status = Command::new(editor).arg(file.path()).status()?;

    if !status.success() {
      bail!("Failed to open temporary file in editor");
    }

    let renamed = fs::read_to_string(file.path())?
      .trim()
      .lines()
      .map(|line| line.to_string())
      .collect::<Vec<String>>();

    if self.paths.len() != renamed.len() {
      bail!(
        "Number of paths changed, should be {}, got {}",
        self.paths.len(),
        renamed.len()
      );
    }

    let mut duplicates = renamed
      .iter()
      .fold(HashMap::new(), |mut acc, v| {
        *acc.entry(v).or_insert(0) += 1;
        acc
      })
      .into_iter()
      .filter(|&(_, count)| count > 1)
      .collect::<Vec<_>>();

    duplicates.sort();

    if !duplicates.is_empty() {
      bail!(
        "Found duplicate destination(s): {}",
        duplicates
          .iter()
          .map(|(path, _)| path.to_string())
          .collect::<Vec<String>>()
          .join(", ")
      );
    }

    let mut pairs = self
      .paths
      .iter()
      .zip(renamed.iter())
      .filter(|(old, new)| old != new)
      .collect::<Vec<_>>();

    let existing = pairs
      .iter()
      .filter(|(_, new)| fs::metadata(new).is_ok())
      .collect::<Vec<_>>();

    if !self.force && !existing.is_empty() {
      bail!(
        "Found destination(s) that already exist: {}, use --force to overwrite",
        existing
          .iter()
          .map(|(_, new)| new.to_string())
          .collect::<Vec<String>>()
          .join(", ")
      );
    }

    let absolutes = renamed
      .iter()
      .map(|path| Path::new(path).absolutize().map_err(anyhow::Error::from))
      .collect::<Result<Vec<_>>>()?;

    let par = absolutes
      .iter()
      .zip(renamed.iter())
      .filter_map(|(path, rename)| path.parent().map(|parent| (parent, rename)))
      .collect::<Vec<_>>();

    let absent = par
      .iter()
      .filter(|(path, _)| !path.exists())
      .map(|(_, rename)| rename.to_string())
      .collect::<Vec<String>>();

    if !absent.is_empty() {
      bail!(
        "Found destination(s) with invalid directory(ies): {}",
        absent.join(", ")
      );
    }

    let indegree = renamed.iter().fold(HashMap::new(), |mut acc, v| {
      (self.paths.contains(v)).then(|| *acc.entry(v).or_insert(0) += 1);
      acc
    });

    pairs.sort_by(|a, b| {
      indegree
        .get(a.1)
        .unwrap_or(&0)
        .cmp(indegree.get(b.1).unwrap_or(&0))
    });

    let mut changed = 0;

    for (old, new) in pairs {
      if !self.dry_run {
        fs::rename(old, new)?;
        changed += 1;
      }

      println!("{old} -> {new}");
    }

    println!("{changed} path(s) changed");

    Ok(())
  }
}

type Result<T = (), E = anyhow::Error> = std::result::Result<T, E>;

fn main() {
  if let Err(error) = Arguments::parse().run() {
    eprintln!("error: {error}");
    process::exit(1);
  }
}
