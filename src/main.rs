use {
  anyhow::bail,
  clap::Parser,
  std::{
    collections::HashMap,
    env, fs,
    io::Write,
    process::{self, Command},
  },
  tempfile::NamedTempFile,
};

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

    for path in &self.paths {
      if !fs::metadata(path).is_ok() {
        bail!("Path does not exist: {}", path);
      }
    }

    let mut file = NamedTempFile::new()?;

    writeln!(file, "{}", self.paths.join("\n"))?;

    let status = Command::new("bash")
      .arg("-c")
      .arg(format!("{} {}", editor, file.path().display()))
      .status()?;

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
        "Duplicate paths found: {}",
        duplicates
          .iter()
          .map(|(path, _)| path.to_string())
          .collect::<Vec<String>>()
          .join(", ")
      );
    }

    let mut changed = 0;

    for (old, new) in self
      .paths
      .iter()
      .zip(renamed.iter())
      .filter(|(old, new)| old != new)
    {
      if !self.dry_run {
        if !self.force && fs::metadata(new).is_ok() {
          println!("Path already exists: {new}, use --force to overwrite");
        } else {
          fs::rename(old, new)?;
          println!("{old} -> {new}");
          changed += 1;
        }
      } else {
        println!("{old} -> {new}");
      }
    }

    println!("{changed} paths changed");

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
