use {
  clap::Parser,
  std::{env, fs, io::Write, process},
  tempfile::NamedTempFile,
};

#[derive(Debug, Parser)]
struct Arguments {
  #[clap(long, help = "Editor command to use")]
  editor: Option<String>,
  #[clap(
    long,
    default_value = "false",
    help = "Run without making any changes"
  )]
  dry_run: bool,
  #[clap(long, default_value = "false", help = "Overwrite existing files")]
  force: bool,
  #[clap(name = "paths", help = "Paths to edit")]
  paths: Vec<String>,
}

impl Arguments {
  fn run(self) -> Result {
    let editor = self
      .editor
      .unwrap_or(env::var("EDITOR").unwrap_or("vim".to_string()));

    for path in &self.paths {
      if !fs::metadata(path).is_ok() {
        anyhow::bail!("Path does not exist: {}", path);
      }
    }

    let mut file = NamedTempFile::new()?;

    writeln!(file, "{}", self.paths.join("\n"))?;

    let status = process::Command::new(editor).arg(file.path()).status()?;

    if !status.success() {
      anyhow::bail!("Failed to open temporary file in editor");
    }

    let renamed = fs::read_to_string(file.path())?
      .trim()
      .lines()
      .map(|line| line.to_string())
      .collect::<Vec<String>>();

    if self.paths.len() != renamed.len() {
      anyhow::bail!(
        "Number of paths changed, should be {}, got {}",
        self.paths.len(),
        renamed.len()
      );
    }

    let duplicates = renamed
      .iter()
      .fold(std::collections::HashMap::new(), |mut acc, v| {
        *acc.entry(v).or_insert(0) += 1;
        acc
      })
      .into_iter()
      .filter(|&(_, count)| count > 1)
      .collect::<Vec<_>>();

    if !duplicates.is_empty() {
      anyhow::bail!(
        "Duplicate paths found: {}",
        duplicates
          .iter()
          .map(|(path, _)| path.to_string())
          .collect::<Vec<String>>()
          .join(", ")
      );
    }

    let mut changed = 0;

    self.paths.iter().zip(renamed.iter()).try_for_each(
      |(old, new)| -> Result {
        println!("{} -> {}", old, new);

        if !self.dry_run {
          if !self.force && fs::metadata(new).is_ok() {
            println!("Path already exists: {new}, use --force to overwrite");
          } else {
            fs::rename(old, new)?;
            changed += 1;
          }
        }

        Ok(())
      },
    )?;

    println!("{changed} paths changed");

    Ok(())
  }
}

type Result<T = (), E = anyhow::Error> = std::result::Result<T, E>;

fn main() {
  if let Err(error) = Arguments::parse().run() {
    eprintln!("erorr: {error}");
    process::exit(1);
  }
}
