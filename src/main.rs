use {
  anyhow::bail,
  clap::Parser,
  path_absolutize::*,
  std::{
    collections::HashMap,
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{self, Command},
  },
  tempfile::{Builder, NamedTempFile, TempDir},
};

#[derive(Debug)]
enum Intermediate {
  File(NamedTempFile),
  Directory(TempDir),
}

impl TryFrom<PathBuf> for Intermediate {
  type Error = anyhow::Error;

  fn try_from(path: PathBuf) -> Result<Self> {
    Ok(match path.is_file() {
      true => Intermediate::File(NamedTempFile::new()?),
      _ => Intermediate::Directory(TempDir::new()?),
    })
  }
}

impl Intermediate {
  fn path(&self) -> &Path {
    match self {
      Intermediate::File(file) => file.path(),
      Intermediate::Directory(dir) => dir.path(),
    }
  }
}

trait PathBufExt {
  fn with(&self, source: &Path) -> Self;
}

impl PathBufExt for PathBuf {
  fn with(&self, source: &Path) -> Self {
    match self.is_dir() {
      true => self.join(source),
      _ => self.to_path_buf(),
    }
  }
}

#[derive(Debug, Parser)]
#[command(about, author, version)]
struct Arguments {
  #[clap(long, help = "Editor command to use")]
  editor: Option<String>,
  #[clap(long, help = "Overwrite existing files")]
  force: bool,
  #[clap(long, help = "Resolve conflicting renames")]
  resolve: bool,
  #[clap(long, help = "Run without making any changes")]
  dry_run: bool,
  #[clap(name = "sources", help = "Paths to edit")]
  sources: Vec<String>,
}

impl Arguments {
  fn run(self) -> Result {
    let editor = self.editor.unwrap_or(
      env::var("EDMV_EDITOR")
        .unwrap_or(env::var("EDITOR").unwrap_or("vi".to_string())),
    );

    let absent = self
      .sources
      .clone()
      .into_iter()
      .filter(|path| fs::metadata(path).is_err())
      .collect::<Vec<String>>();

    if !absent.is_empty() {
      bail!("Found non-existent path(s): {}", absent.join(", "));
    }

    let mut file = Builder::new()
      .prefix(&format!("{}-", env!("CARGO_PKG_NAME")))
      .suffix(".txt")
      .tempfile()?;

    writeln!(file, "{}", &self.sources.join("\n"))?;

    let status = Command::new(editor).arg(file.path()).status()?;

    if !status.success() {
      bail!("Failed to open temporary file in editor");
    }

    let destinations = fs::read_to_string(file.path())?
      .trim()
      .lines()
      .map(|line| line.to_string())
      .collect::<Vec<String>>();

    if self.sources.len() != destinations.len() {
      bail!(
        "Destination count mismatch, should be {} but received {}",
        self.sources.len(),
        destinations.len()
      );
    }

    let mut duplicates = destinations
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

    let pairs = self
      .sources
      .iter()
      .zip(destinations.iter())
      .filter(|(source, destination)| source != destination)
      .map(|(source, destination)| {
        (PathBuf::from(source), PathBuf::from(destination))
      })
      .collect::<Vec<(PathBuf, PathBuf)>>();

    let existing = pairs
      .iter()
      .filter(|(_, destination)| fs::metadata(destination).is_ok())
      .map(|(_, destination)| destination.display())
      .collect::<Vec<_>>();

    if !self.force && !existing.is_empty() {
      bail!(
        "Found destination(s) that already exist: {}, use --force to overwrite",
        existing
          .iter()
          .map(|path| path.to_string())
          .collect::<Vec<String>>()
          .join(", ")
      );
    }

    let map = pairs.iter().cloned().collect::<HashMap<PathBuf, PathBuf>>();

    let mut conflicting = map
      .iter()
      .filter(|(_, destination)| map.contains_key(destination.to_owned()))
      .map(|(source, destination)| {
        format!("{} -> {}", source.display(), destination.display())
      })
      .collect::<Vec<String>>();

    conflicting.sort();

    if !conflicting.is_empty() && !self.resolve {
      bail!(
        "Found conflicting operation(s): {}, use --resolve to properly handle the conflicts",
        conflicting.join(", ")
      );
    }

    let dir_to_file = pairs
      .iter()
      .filter(|(source, destination)| source.is_dir() && destination.is_file())
      .map(|(source, destination)| {
        format!("{} -> {}", source.display(), destination.display())
      })
      .collect::<Vec<_>>();

    if !dir_to_file.is_empty() {
      bail!(
        "Found directory to file operation(s): {}",
        dir_to_file.join(", ")
      );
    }

    let absolutes = pairs
      .iter()
      .map(|(_, destination)| {
        destination.absolutize().map_err(anyhow::Error::from)
      })
      .collect::<Result<Vec<_>>>()?;

    let par = absolutes
      .iter()
      .zip(destinations.iter())
      .filter_map(|(path, destination)| {
        path.parent().map(|parent| (parent, destination))
      })
      .collect::<Vec<_>>();

    let absent = par
      .iter()
      .filter(|(path, _)| !path.exists())
      .map(|(_, destination)| destination.to_string())
      .collect::<Vec<String>>();

    if !absent.is_empty() {
      bail!(
        "Found destination(s) placed within a non-existent directory: {}",
        absent.join(", ")
      );
    }

    let mut changed = 0;

    let intermediates = self.resolve.then_some(
      self
        .sources
        .iter()
        .map(|path| Intermediate::try_from(PathBuf::from(path)))
        .collect::<Result<Vec<_>>>()?,
    );

    match intermediates {
      Some(intermediates) => {
        let combined = pairs
          .iter()
          .zip(intermediates.iter())
          .map(|((source, destination), intermediate)| {
            (source, intermediate, destination)
          })
          .collect::<Vec<_>>();

        if !self.dry_run {
          combined.iter().try_for_each(|(source, intermediate, _)| {
            fs::rename(source, intermediate.path())
          })?;
        }

        combined.iter().try_for_each(
          |(source, intermediate, destination)| -> Result {
            let destination = destination.with(source);

            if !self.dry_run {
              fs::rename(intermediate.path(), &destination)?;
              changed += 1;
            }

            println!("{} -> {}", source.display(), destination.display());

            Ok(())
          },
        )
      }
      None => pairs
        .iter()
        .try_for_each(|(source, destination)| -> Result {
          let destination = destination.with(source);

          if !self.dry_run {
            fs::rename(source, &destination)?;
            changed += 1;
          }

          println!("{} -> {}", source.display(), destination.display());

          Ok(())
        }),
    }?;

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
