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

struct Source<'a> {
  from: &'a str,
  to: &'a str,
  create: bool,
  exists: Vec<&'a str>,
}

struct Test<'a> {
  arguments: Vec<String>,
  expected_status: i32,
  expected_stderr: String,
  expected_stdout: String,
  sources: Vec<Source<'a>>,
  tempdir: TempDir,
}

impl<'a> Test<'a> {
  fn new() -> Result<Self> {
    Ok(Self {
      arguments: Vec::new(),
      expected_status: 0,
      expected_stderr: String::new(),
      expected_stdout: String::new(),
      sources: Vec::new(),
      tempdir: TempDir::new()?,
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

  fn sources(self, sources: Vec<Source<'a>>) -> Self {
    Self { sources, ..self }
  }

  fn create(self, path: &str) -> Result<Self> {
    File::create(self.tempdir.path().join(path))?;
    Ok(self)
  }

  fn run(self) -> Result {
    self.run_and_return_tempdir().map(|_| ())
  }

  fn command(&self) -> Result<Command> {
    let mut command = Command::new(executable_path(env!("CARGO_PKG_NAME")));

    self
      .sources
      .iter()
      .filter(|path| path.create)
      .try_for_each(|path| -> Result {
        File::create(self.tempdir.path().join(path.from))?;
        Ok(())
      })?;

    let editor = self.tempdir.path().join("editor");

    fs::write(
      &editor,
      format!(
        "#!/bin/bash\necho -e \"{}\" > \"$1\"",
        self
          .sources
          .iter()
          .map(|path| path.to)
          .collect::<Vec<_>>()
          .join("\n")
      ),
    )?;

    fs::set_permissions(&editor, Permissions::from_mode(0o755))?;

    command
      .current_dir(&self.tempdir)
      .args(self.sources.iter().map(|path| path.from))
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

    let exists = self
      .sources
      .iter()
      .flat_map(|path| path.exists.to_owned())
      .collect::<Vec<_>>();

    self
      .sources
      .iter()
      .flat_map(|path| vec![path.from, path.to])
      .for_each(|option| {
        assert_eq!(
          exists.contains(&option),
          self.tempdir.path().join(option).exists()
        );
      });

    Ok(self.tempdir)
  }
}

#[test]
fn renames_non_existing_sources() -> Result {
  Test::new()?
    .sources(vec![
      Source {
        from: "a.txt",
        to: "d.txt",
        create: true,
        exists: vec!["d.txt"],
      },
      Source {
        from: "b.txt",
        to: "e.txt",
        create: true,
        exists: vec!["e.txt"],
      },
      Source {
        from: "c.txt",
        to: "f.txt",
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
      3 path(s) changed
      ",
    )
    .run()
}

#[test]
fn gives_error_for_existing_destinations() -> Result {
  Test::new()?
    .sources(vec![
      Source {
        from: "a.txt",
        to: "d.txt",
        create: true,
        exists: vec!["a.txt", "d.txt"],
      },
      Source {
        from: "b.txt",
        to: "e.txt",
        create: true,
        exists: vec!["b.txt", "e.txt"],
      },
      Source {
        from: "c.txt",
        to: "f.txt",
        create: true,
        exists: vec!["c.txt"],
      },
    ])
    .create("d.txt")?
    .create("e.txt")?
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
    .sources(vec![
      Source {
        from: "a.txt",
        to: "d.txt",
        create: true,
        exists: vec!["d.txt"],
      },
      Source {
        from: "b.txt",
        to: "e.txt",
        create: true,
        exists: vec!["e.txt"],
      },
      Source {
        from: "c.txt",
        to: "f.txt",
        create: true,
        exists: vec!["f.txt"],
      },
    ])
    .argument("--force")
    .create("d.txt")?
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
    .sources(vec![
      Source {
        from: "a.txt",
        to: "d.txt",
        create: true,
        exists: vec!["a.txt"],
      },
      Source {
        from: "b.txt",
        to: "e.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Source {
        from: "c.txt",
        to: "f.txt",
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
      0 path(s) changed
      ",
    )
    .run()
}

#[test]
fn errors_when_passed_invalid_sources() -> Result {
  Test::new()?
    .sources(vec![
      Source {
        from: "a.txt",
        to: "b.txt",
        create: false,
        exists: vec![],
      },
      Source {
        from: "c.txt",
        to: "b.txt",
        create: false,
        exists: vec![],
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
fn disallow_duplicate_sources() -> Result {
  Test::new()?
    .sources(vec![
      Source {
        from: "a.txt",
        to: "c.txt",
        create: true,
        exists: vec!["a.txt"],
      },
      Source {
        from: "b.txt",
        to: "c.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Source {
        from: "e.txt",
        to: "f.txt",
        create: true,
        exists: vec!["e.txt"],
      },
      Source {
        from: "e.txt",
        to: "f.txt",
        create: true,
        exists: vec!["e.txt"],
      },
    ])
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
    .sources(vec![
      Source {
        from: "a.txt",
        to: "b.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Source {
        from: "b.txt",
        to: "c.txt",
        create: true,
        exists: vec!["c.txt", "b.txt"],
      },
      Source {
        from: "d.txt",
        to: "e.txt",
        create: true,
        exists: vec!["e.txt"],
      },
    ])
    .argument("--force")
    .argument("--resolve")
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
    .sources(vec![
      Source {
        from: "a.txt",
        to: "a.txt",
        create: true,
        exists: vec!["a.txt"],
      },
      Source {
        from: "b.txt",
        to: "b.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Source {
        from: "c.txt",
        to: "c.txt",
        create: true,
        exists: vec!["c.txt"],
      },
    ])
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
    .sources(vec![
      Source {
        from: "a.txt",
        to: "foo/a.txt",
        create: true,
        exists: vec!["a.txt"],
      },
      Source {
        from: "b.txt",
        to: "bar/baz/c.txt",
        create: true,
        exists: vec!["b.txt"],
      },
    ])
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
    .sources(vec![
      Source {
        from: "a.txt",
        to: "b.txt",
        create: true,
        exists: vec!["a.txt", "b.txt"],
      },
      Source {
        from: "b.txt",
        to: "a.txt",
        create: true,
        exists: vec!["b.txt", "a.txt"],
      },
    ])
    .argument("--force")
    .argument("--resolve")
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
    .sources(vec![
      Source {
        from: "a.txt",
        to: "a.txt",
        create: true,
        exists: vec!["a.txt"],
      },
      Source {
        from: "b.txt",
        to: "b.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Source {
        from: "c.txt",
        to: "d.txt",
        create: true,
        exists: vec!["d.txt"],
      },
    ])
    .expected_status(0)
    .expected_stdout(
      "
      c.txt -> d.txt
      1 path(s) changed
      ",
    )
    .run()
}
