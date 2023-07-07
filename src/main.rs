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
  tempdir::TempDir,
  tempfile::{Builder, NamedTempFile},
};

#[derive(Debug, Parser)]
struct Arguments {
  #[clap(long, help = "Editor command to use")]
  editor: Option<String>,
  #[clap(long, help = "Overwrite existing files")]
  force: bool,
  #[clap(long, help = "Rename sources to temporary files internally")]
  temp: bool,
  #[clap(long, help = "Run without making any changes")]
  dry_run: bool,
  #[clap(name = "sources", help = "Paths to edit")]
  sources: Vec<String>,
}

enum Intermediate {
  File(NamedTempFile),
  Directory(TempDir),
}

impl TryFrom<PathBuf> for Intermediate {
  type Error = anyhow::Error;

  fn try_from(path: PathBuf) -> Result<Self> {
    Ok(match path.is_file() {
      true => Intermediate::File(NamedTempFile::new()?),
      _ => Intermediate::Directory(TempDir::new(env!("CARGO_PKG_NAME"))?),
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

impl Arguments {
  fn run(self) -> Result {
    let editor = self
      .editor
      .unwrap_or(env::var("EDITOR").unwrap_or("vi".to_string()));

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
        "Number of sources changed, should be {}, got {}",
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

    let existing = destinations
      .iter()
      .filter(|path| fs::metadata(path).is_ok())
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

    let absolutes = destinations
      .iter()
      .map(|path| Path::new(path).absolutize().map_err(anyhow::Error::from))
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
        "Found destination(s) with non-existent directory(ies): {}",
        absent.join(", ")
      );
    }

    let pairs = self
      .sources
      .iter()
      .zip(destinations.iter())
      .filter(|(source, destination)| source != destination)
      .collect::<Vec<(&String, &String)>>();

    let mut changed = 0;

    if self.temp {
      let intermediates = self
        .sources
        .iter()
        .map(|path| Intermediate::try_from(PathBuf::from(path)))
        .collect::<Result<Vec<_>>>()?;

      let zipped = pairs
        .iter()
        .zip(intermediates.iter())
        .map(|((source, destination), internal)| {
          (*source, internal, *destination)
        })
        .collect::<Vec<_>>();

      for (source, intermediate, _) in &zipped {
        if !self.dry_run {
          fs::rename(source, intermediate.path())?;
        }
      }

      for (source, intermediate, destination) in &zipped {
        if !self.dry_run {
          fs::rename(intermediate.path(), destination)?;
          changed += 1;
        }

        println!("{} -> {}", source, destination);
      }
    } else {
      for (source, destination) in pairs {
        if !self.dry_run {
          fs::rename(source, destination)?;
          changed += 1;
        }

        println!("{} -> {}", source, destination);
      }
    }

    println!("{} path(s) changed", changed);

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
