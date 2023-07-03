use {
  executable_path::executable_path,
  pretty_assertions::assert_eq,
  std::{fs::File, path::PathBuf, process::Command, str},
  tempdir::TempDir,
  unindent::Unindent,
};

type Result<T = (), E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

struct Path<'a> {
  old: &'a str,
  new: &'a str,
  rename: bool,
  create: bool,
}

struct Test<'a> {
  arguments: Vec<String>,
  expected_status: i32,
  expected_stderr: String,
  expected_stdout: String,
  paths: Vec<Path<'a>>,
  tempdir: TempDir,
}

impl<'a> Test<'a> {
  fn new() -> Result<Self> {
    Ok(Self {
      arguments: Vec::new(),
      expected_status: 0,
      expected_stderr: String::new(),
      expected_stdout: String::new(),
      paths: Vec::new(),
      tempdir: TempDir::new("test")?,
    })
  }

  fn argument(mut self, argument: &str) -> Self {
    self.arguments.push(argument.to_owned());
    self
  }

  fn expected_status(self, expected_status: i32) -> Self {
    Self {
      expected_status,
      ..self
    }
  }

  fn expected_stderr(self, expected_stderr: &str) -> Self {
    Self {
      expected_stderr: expected_stderr.unindent(),
      ..self
    }
  }

  fn expected_stdout(self, expected_stdout: &str) -> Self {
    Self {
      expected_stdout: expected_stdout.unindent(),
      ..self
    }
  }

  fn paths(self, paths: Vec<Path<'a>>) -> Self {
    Self { paths, ..self }
  }

  fn create_file(self, path: PathBuf) -> Result<Self> {
    File::create(self.tempdir.path().join(path))?;
    Ok(self)
  }

  fn run(self) -> Result {
    self.run_and_return_tempdir().map(|_| ())
  }

  fn command(&self) -> Result<Command> {
    let mut command = Command::new(executable_path(env!("CARGO_PKG_NAME")));

    let old = self
      .paths
      .iter()
      .map(|path| (path.old, path.create))
      .collect::<Vec<_>>();

    for (path, create) in &old {
      if *create {
        File::create(self.tempdir.path().join(path))?;
      }
    }

    let editor = format!(
      "echo -e '{}' >",
      self
        .paths
        .iter()
        .map(|path| path.new)
        .collect::<Vec<_>>()
        .join("\n")
    );

    command
      .current_dir(&self.tempdir)
      .args(old.iter().map(|(path, _)| path))
      .arg("--editor")
      .arg(editor)
      .args(self.arguments.clone());

    Ok(command)
  }

  fn run_and_return_tempdir(self) -> Result<TempDir> {
    let output = self.command()?.output()?;

    assert_eq!(output.status.code(), Some(self.expected_status));

    let stderr = str::from_utf8(&output.stderr)?;
    let stdout = str::from_utf8(&output.stdout)?;

    if self.expected_stderr.is_empty() && !stderr.is_empty() {
      panic!("Expected empty stderr, but received: {}", stderr);
    } else {
      assert_eq!(stderr, self.expected_stderr);
    }

    assert_eq!(stdout, self.expected_stdout);

    let exists = |name: &str| self.tempdir.path().join(name).exists();

    for path in self.paths {
      if !path.create {
        assert!(!exists(path.old));
        assert!(!exists(path.new));
      } else {
        if path.rename {
          assert!(!exists(path.old));
          assert!(exists(path.new));
        } else {
          assert!(exists(path.old));
          assert!(!exists(path.new));
        }
      }
    }

    Ok(self.tempdir)
  }
}

#[test]
fn renames_non_existing_file_paths() -> Result {
  Test::new()?
    .paths(vec![
      Path {
        old: "a.txt",
        new: "d.txt",
        rename: true,
        create: true,
      },
      Path {
        old: "b.txt",
        new: "e.txt",
        rename: true,
        create: true,
      },
      Path {
        old: "c.txt",
        new: "f.txt",
        rename: true,
        create: true,
      },
    ])
    .expected_status(0)
    .expected_stdout(
      "
      a.txt -> d.txt
      b.txt -> e.txt
      c.txt -> f.txt
      3 paths changed
      ",
    )
    .run()
}

#[test]
fn gives_warning_for_existing_file_paths() -> Result {
  Test::new()?
    .paths(vec![
      Path {
        old: "a.txt",
        new: "b.txt",
        rename: false,
        create: true,
      },
      Path {
        old: "b.txt",
        new: "e.txt",
        rename: true,
        create: true,
      },
      Path {
        old: "c.txt",
        new: "f.txt",
        rename: true,
        create: true,
      },
    ])
    .expected_status(0)
    .expected_stdout(
      "
      Path already exists: b.txt, use --force to overwrite
      b.txt -> e.txt
      c.txt -> f.txt
      2 paths changed
      ",
    )
    .run()
}

#[test]
fn forces_existing_file_paths() -> Result {
  Test::new()?
    .paths(vec![
      Path {
        old: "a.txt",
        new: "d.txt",
        rename: true,
        create: true,
      },
      Path {
        old: "b.txt",
        new: "e.txt",
        rename: true,
        create: true,
      },
      Path {
        old: "c.txt",
        new: "f.txt",
        rename: true,
        create: true,
      },
    ])
    .create_file("d.txt".into())?
    .expected_status(0)
    .expected_stdout(
      "
      a.txt -> d.txt
      b.txt -> e.txt
      c.txt -> f.txt
      3 paths changed
      ",
    )
    .argument("--force")
    .run()
}

#[test]
fn dry_run_works() -> Result {
  Test::new()?
    .paths(vec![
      Path {
        old: "a.txt",
        new: "d.txt",
        rename: false,
        create: true,
      },
      Path {
        old: "b.txt",
        new: "e.txt",
        rename: false,
        create: true,
      },
      Path {
        old: "c.txt",
        new: "f.txt",
        rename: false,
        create: true,
      },
    ])
    .argument("--dry-run")
    .expected_status(0)
    .expected_stdout(
      "
      a.txt -> d.txt
      b.txt -> e.txt
      c.txt -> f.txt
      0 paths changed
      ",
    )
    .run()
}

#[test]
fn errors_when_passed_invalid_path() -> Result {
  Test::new()?
    .paths(vec![Path {
      old: "a.txt",
      new: "b.txt",
      rename: false,
      create: false,
    }])
    .expected_status(1)
    .expected_stderr(
      "
      error: Path does not exist: a.txt
      ",
    )
    .run()
}

#[test]
fn disallow_duplicate_paths() -> Result {
  Test::new()?
    .paths(vec![
      Path {
        old: "a.txt",
        new: "c.txt",
        rename: false,
        create: true,
      },
      Path {
        old: "b.txt",
        new: "c.txt",
        rename: false,
        create: true,
      },
      Path {
        old: "e.txt",
        new: "f.txt",
        rename: false,
        create: true,
      },
      Path {
        old: "e.txt",
        new: "f.txt",
        rename: false,
        create: true,
      },
    ])
    .expected_status(1)
    .expected_stderr(
      "
      error: Duplicate paths found: c.txt, f.txt
      ",
    )
    .run()
}
