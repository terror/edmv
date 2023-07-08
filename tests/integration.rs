use {
  executable_path::executable_path,
  pretty_assertions::assert_eq,
  std::{
    fs::{self, File, Permissions},
    os::unix::fs::PermissionsExt,
    process::Command,
    str,
  },
  tempfile::TempDir,
  unindent::Unindent,
};

type Result<T = (), E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

#[allow(dead_code)]
enum Path<'a> {
  File(&'a str),
  Directory(&'a str),
}

impl Path<'_> {
  fn create(&self, tempdir: &TempDir) -> Result {
    match self {
      Self::File(path) => {
        File::create(tempdir.path().join(path))?;
        Ok(())
      }
      Self::Directory(path) => {
        fs::create_dir(tempdir.path().join(path))?;
        Ok(())
      }
    }
  }
}

#[derive(Clone)]
struct Operation<'a> {
  source: &'a str,
  destination: Option<&'a str>,
}

struct Test<'a> {
  arguments: Vec<String>,
  exists: Vec<&'a str>,
  expected_status: i32,
  expected_stderr: String,
  expected_stdout: String,
  operations: Vec<Operation<'a>>,
  tempdir: TempDir,
}

impl<'a> Test<'a> {
  fn new() -> Result<Self> {
    Ok(Self {
      arguments: Vec::new(),
      exists: Vec::new(),
      expected_status: 0,
      expected_stderr: String::new(),
      expected_stdout: String::new(),
      operations: Vec::new(),
      tempdir: TempDir::new()?,
    })
  }

  fn argument(mut self, argument: &str) -> Self {
    self.arguments.push(argument.to_owned());
    self
  }

  fn exists(self, exists: &[&'a str]) -> Self {
    Self {
      exists: exists.to_vec(),
      ..self
    }
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

  fn operations(self, operations: &[Operation<'a>]) -> Self {
    Self {
      operations: operations.to_vec(),
      ..self
    }
  }

  fn create(self, paths: &[Path]) -> Result<Self> {
    paths
      .iter()
      .try_for_each(|path| path.create(&self.tempdir))?;

    Ok(self)
  }

  fn run(self) -> Result {
    self.run_and_return_tempdir().map(|_| ())
  }

  fn command(&self) -> Result<Command> {
    let mut command = Command::new(executable_path(env!("CARGO_PKG_NAME")));

    let editor = self.tempdir.path().join("editor");

    fs::write(
      &editor,
      format!(
        "#!/bin/bash\necho -e \"{}\" > \"$1\"",
        self
          .operations
          .iter()
          .filter_map(|path| path.destination)
          .collect::<Vec<_>>()
          .join("\n")
      ),
    )?;

    fs::set_permissions(&editor, Permissions::from_mode(0o755))?;

    command
      .current_dir(&self.tempdir)
      .args(self.operations.iter().map(|path| path.source))
      .arg("--editor")
      .arg(editor)
      .args(&self.arguments);

    Ok(command)
  }

  fn run_and_return_tempdir(self) -> Result<TempDir> {
    let output = self.command()?.output()?;

    assert_eq!(output.status.code(), Some(self.expected_status));

    let stderr = str::from_utf8(&output.stderr)?;

    if self.expected_stderr.is_empty() && !stderr.is_empty() {
      panic!("Expected empty stderr, but received: {}", stderr);
    } else {
      assert_eq!(stderr, self.expected_stderr);
    }

    assert_eq!(str::from_utf8(&output.stdout)?, self.expected_stdout);

    let sources = self
      .operations
      .iter()
      .map(|operation| operation.source)
      .collect::<Vec<_>>();

    let destinations = self
      .operations
      .iter()
      .flat_map(|operation| operation.destination)
      .collect::<Vec<_>>();

    let combined = sources
      .iter()
      .chain(destinations.iter())
      .collect::<Vec<_>>();

    combined.iter().for_each(|path| {
      assert_eq!(
        self.exists.contains(path),
        self.tempdir.path().join(path).exists()
      );
    });

    self
      .exists
      .iter()
      .filter(|path| !combined.contains(path))
      .for_each(|path| {
        assert!(self.tempdir.path().join(path).exists());
      });

    Ok(self.tempdir)
  }
}

#[test]
fn renames_non_existing_operations() -> Result {
  Test::new()?
    .create(&[
      Path::File("a.txt"),
      Path::File("b.txt"),
      Path::File("c.txt"),
    ])?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: Some("d.txt"),
      },
      Operation {
        source: "b.txt",
        destination: Some("e.txt"),
      },
      Operation {
        source: "c.txt",
        destination: Some("f.txt"),
      },
    ])
    .exists(&["d.txt", "e.txt", "f.txt"])
    .expected_status(0)
    .expected_stdout(
      "
      a.txt -> d.txt
      b.txt -> e.txt
      c.txt -> f.txt
      3 path(s) changed
      ",
    )
    .run()
}

#[test]
fn gives_error_for_existing_destinations() -> Result {
  Test::new()?
    .create(&[
      Path::File("a.txt"),
      Path::File("b.txt"),
      Path::File("c.txt"),
      Path::File("d.txt"),
      Path::File("e.txt")
    ])?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: Some("d.txt"),
      },
      Operation {
        source: "b.txt",
        destination: Some("e.txt"),
      },
      Operation {
        source: "c.txt",
        destination: Some("f.txt"),
      },
    ])
    .exists(&["a.txt", "b.txt", "c.txt", "d.txt", "e.txt"])
    .expected_status(1)
    .expected_stderr(
      "
      error: Found destination(s) that already exist: d.txt, e.txt, use --force to overwrite
      ",
    )
    .run()
}

#[test]
fn forces_existing_destinations() -> Result {
  Test::new()?
    .argument("--force")
    .create(&[
      Path::File("a.txt"),
      Path::File("b.txt"),
      Path::File("c.txt"),
      Path::File("d.txt"),
    ])?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: Some("d.txt"),
      },
      Operation {
        source: "b.txt",
        destination: Some("e.txt"),
      },
      Operation {
        source: "c.txt",
        destination: Some("f.txt"),
      },
    ])
    .exists(&["d.txt", "e.txt", "f.txt"])
    .expected_status(0)
    .expected_stdout(
      "
      a.txt -> d.txt
      b.txt -> e.txt
      c.txt -> f.txt
      3 path(s) changed
      ",
    )
    .run()
}

#[test]
fn dry_run_works() -> Result {
  Test::new()?
    .argument("--dry-run")
    .create(&[
      Path::File("a.txt"),
      Path::File("b.txt"),
      Path::File("c.txt"),
    ])?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: Some("d.txt"),
      },
      Operation {
        source: "b.txt",
        destination: Some("e.txt"),
      },
      Operation {
        source: "c.txt",
        destination: Some("f.txt"),
      },
    ])
    .exists(&["a.txt", "b.txt", "c.txt"])
    .expected_status(0)
    .expected_stdout(
      "
      a.txt -> d.txt
      b.txt -> e.txt
      c.txt -> f.txt
      0 path(s) changed
      ",
    )
    .run()
}

#[test]
fn errors_when_passed_invalid_operations() -> Result {
  Test::new()?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: Some("b.txt"),
      },
      Operation {
        source: "c.txt",
        destination: Some("b.txt"),
      },
    ])
    .expected_status(1)
    .expected_stderr(
      "
      error: Found non-existent path(s): a.txt, c.txt
      ",
    )
    .run()
}

#[test]
fn disallow_duplicate_operations() -> Result {
  Test::new()?
    .create(&[
      Path::File("a.txt"),
      Path::File("b.txt"),
      Path::File("e.txt"),
    ])?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: Some("c.txt"),
      },
      Operation {
        source: "b.txt",
        destination: Some("c.txt"),
      },
      Operation {
        source: "e.txt",
        destination: Some("f.txt"),
      },
      Operation {
        source: "e.txt",
        destination: Some("f.txt"),
      },
    ])
    .exists(&["a.txt", "b.txt", "e.txt"])
    .expected_status(1)
    .expected_stderr(
      "
      error: Found duplicate destination(s): c.txt, f.txt
      ",
    )
    .run()
}

#[test]
fn handles_intermediate_conflicts() -> Result {
  Test::new()?
    .argument("--force")
    .argument("--resolve")
    .create(&[
      Path::File("a.txt"),
      Path::File("b.txt"),
      Path::File("d.txt"),
    ])?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: Some("b.txt"),
      },
      Operation {
        source: "b.txt",
        destination: Some("c.txt"),
      },
      Operation {
        source: "d.txt",
        destination: Some("e.txt"),
      },
    ])
    .exists(&["b.txt", "c.txt", "e.txt"])
    .expected_status(0)
    .expected_stdout(
      "
      a.txt -> b.txt
      b.txt -> c.txt
      d.txt -> e.txt
      3 path(s) changed
      ",
    )
    .run()
}

#[test]
fn does_not_perform_self_renames() -> Result {
  Test::new()?
    .create(&[
      Path::File("a.txt"),
      Path::File("b.txt"),
      Path::File("c.txt"),
    ])?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: Some("a.txt"),
      },
      Operation {
        source: "b.txt",
        destination: Some("b.txt"),
      },
      Operation {
        source: "c.txt",
        destination: Some("c.txt"),
      },
    ])
    .exists(&["a.txt", "b.txt", "c.txt"])
    .expected_status(0)
    .expected_stdout(
      "
      0 path(s) changed
      ",
    )
    .run()
}

#[test]
fn gives_error_for_invalid_destination_directory() -> Result {
  Test::new()?
    .create(&[
      Path::File("a.txt"),
      Path::File("b.txt"),
    ])?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: Some("foo/a.txt"),
      },
      Operation {
        source: "b.txt",
        destination: Some("bar/baz/c.txt"),
      },
    ])
    .exists(&["a.txt", "b.txt"])
    .expected_status(1)
    .expected_stderr(
      "
      error: Found destination(s) placed within a non-existent directory: foo/a.txt, bar/baz/c.txt
      ",
    )
    .run()
}

#[test]
fn circular_rename() -> Result {
  Test::new()?
    .argument("--force")
    .argument("--resolve")
    .create(&[Path::File("a.txt"), Path::File("b.txt")])?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: Some("b.txt"),
      },
      Operation {
        source: "b.txt",
        destination: Some("a.txt"),
      },
    ])
    .exists(&["a.txt", "b.txt"])
    .expected_status(0)
    .expected_stdout(
      "
      a.txt -> b.txt
      b.txt -> a.txt
      2 path(s) changed
      ",
    )
    .run()
}

#[test]
fn mixed_self_and_proper_renames() -> Result {
  Test::new()?
    .create(&[
      Path::File("a.txt"),
      Path::File("b.txt"),
      Path::File("c.txt"),
    ])?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: Some("a.txt"),
      },
      Operation {
        source: "b.txt",
        destination: Some("b.txt"),
      },
      Operation {
        source: "c.txt",
        destination: Some("d.txt"),
      },
    ])
    .exists(&["a.txt", "b.txt", "d.txt"])
    .expected_status(0)
    .expected_stdout(
      "
      c.txt -> d.txt
      1 path(s) changed
      ",
    )
    .run()
}

#[test]
fn destination_count_mismatch() -> Result {
  Test::new()?
    .create(&[Path::File("a.txt"), Path::File("b.txt")])?
    .operations(&[
      Operation {
        source: "a.txt",
        destination: None,
      },
      Operation {
        source: "b.txt",
        destination: Some("c.txt"),
      },
    ])
    .exists(&["a.txt", "b.txt"])
    .expected_status(1)
    .expected_stderr(
      "
      error: Destination count mismatch, should be 2 but received 1
      ",
    )
    .run()
}
