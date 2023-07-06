use {
  executable_path::executable_path,
  pretty_assertions::assert_eq,
  std::{
    fs::{self, File, Permissions},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    str,
  },
  tempdir::TempDir,
  unindent::Unindent,
};

type Result<T = (), E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

struct Path<'a> {
  old: &'a str,
  new: &'a str,
  create: bool,
  exists: Vec<&'a str>,
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

    for path in &self.paths {
      if path.create {
        File::create(self.tempdir.path().join(path.old))?;
      }
    }

    let editor = self.tempdir.path().join("editor");

    fs::write(
      &editor,
      format!(
        "#!/bin/bash\necho -e \"{}\" > \"$1\"",
        self
          .paths
          .iter()
          .map(|path| path.new)
          .collect::<Vec<_>>()
          .join("\n")
      ),
    )?;

    fs::set_permissions(&editor, Permissions::from_mode(0o755))?;

    command
      .current_dir(&self.tempdir)
      .args(self.paths.iter().map(|path| path.old))
      .arg("--editor")
      .arg(editor)
      .args(&self.arguments);

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
      for option in &[path.old, path.new] {
        if path.exists.contains(option) {
          assert!(exists(option));
        } else {
          assert!(!exists(option));
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
        create: true,
        exists: vec!["d.txt"],
      },
      Path {
        old: "b.txt",
        new: "e.txt",
        create: true,
        exists: vec!["e.txt"],
      },
      Path {
        old: "c.txt",
        new: "f.txt",
        create: true,
        exists: vec!["f.txt"],
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
fn gives_error_for_existing_destinations() -> Result {
  Test::new()?
    .paths(vec![
      Path {
        old: "a.txt",
        new: "d.txt",
        create: true,
        exists: vec!["a.txt", "d.txt"],
      },
      Path {
        old: "b.txt",
        new: "e.txt",
        create: true,
        exists: vec!["b.txt", "e.txt"],
      },
      Path {
        old: "c.txt",
        new: "f.txt",
        create: true,
        exists: vec!["c.txt"],
      },
    ])
    .create_file("d.txt".into())?
    .create_file("e.txt".into())?
    .expected_status(1)
    .expected_stderr(
      "
      error: Destination(s) already exist: d.txt, e.txt, use --force to overwrite
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
        create: true,
        exists: vec!["d.txt"],
      },
      Path {
        old: "b.txt",
        new: "e.txt",
        create: true,
        exists: vec!["e.txt"],
      },
      Path {
        old: "c.txt",
        new: "f.txt",
        create: true,
        exists: vec!["f.txt"],
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
        create: true,
        exists: vec!["a.txt"],
      },
      Path {
        old: "b.txt",
        new: "e.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Path {
        old: "c.txt",
        new: "f.txt",
        create: true,
        exists: vec!["c.txt"],
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
fn errors_when_passed_invalid_paths() -> Result {
  Test::new()?
    .paths(vec![
      Path {
        old: "a.txt",
        new: "b.txt",
        create: false,
        exists: vec![],
      },
      Path {
        old: "c.txt",
        new: "b.txt",
        create: false,
        exists: vec![],
      },
    ])
    .expected_status(1)
    .expected_stderr(
      "
      error: Found path(s) that do not exist: a.txt, c.txt
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
        create: true,
        exists: vec!["a.txt"],
      },
      Path {
        old: "b.txt",
        new: "c.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Path {
        old: "e.txt",
        new: "f.txt",
        create: true,
        exists: vec!["e.txt"],
      },
      Path {
        old: "e.txt",
        new: "f.txt",
        create: true,
        exists: vec!["e.txt"],
      },
    ])
    .expected_status(1)
    .expected_stderr(
      "
      error: Duplicate destination(s) found: c.txt, f.txt
      ",
    )
    .run()
}

#[test]
fn sorts_by_indegree() -> Result {
  Test::new()?
    .paths(vec![
      Path {
        old: "a.txt",
        new: "b.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Path {
        old: "b.txt",
        new: "c.txt",
        create: true,
        exists: vec!["c.txt", "b.txt"],
      },
      Path {
        old: "d.txt",
        new: "e.txt",
        create: true,
        exists: vec!["e.txt"],
      },
    ])
    .expected_status(0)
    .expected_stdout(
      "
      b.txt -> c.txt
      d.txt -> e.txt
      a.txt -> b.txt
      3 paths changed
      ",
    )
    .argument("--force")
    .run()
}

#[test]
fn does_not_perform_self_renames() -> Result {
  Test::new()?
    .paths(vec![
      Path {
        old: "a.txt",
        new: "a.txt",
        create: true,
        exists: vec!["a.txt"],
      },
      Path {
        old: "b.txt",
        new: "b.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Path {
        old: "c.txt",
        new: "c.txt",
        create: true,
        exists: vec!["c.txt"],
      },
    ])
    .expected_status(0)
    .expected_stdout(
      "
      0 paths changed
      ",
    )
    .argument("--force")
    .run()
}
