use {
  executable_path::executable_path,
  pretty_assertions::assert_eq,
  std::{
    fs::{self, File, Permissions},
    os::unix::fs::PermissionsExt,
    process::Command,
    str,
  },
  tempdir::TempDir,
  unindent::Unindent,
};

type Result<T = (), E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

struct Path<'a> {
  source: &'a str,
  destination: &'a str,
  create: bool,
  exists: Vec<&'a str>,
}

struct Test<'a> {
  arguments: Vec<String>,
  expected_status: i32,
  expected_stderr: String,
  expected_stdout: String,
  sources: Vec<Path<'a>>,
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

  fn sources(self, sources: Vec<Path<'a>>) -> Self {
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

    for path in &self.sources {
      if path.create {
        File::create(self.tempdir.path().join(path.source))?;
      }
    }

    let editor = self.tempdir.path().join("editor");

    fs::write(
      &editor,
      format!(
        "#!/bin/bash\necho -e \"{}\" > \"$1\"",
        self
          .sources
          .iter()
          .map(|path| path.destination)
          .collect::<Vec<_>>()
          .join("\n")
      ),
    )?;

    fs::set_permissions(&editor, Permissions::from_mode(0o755))?;

    command
      .current_dir(&self.tempdir)
      .args(self.sources.iter().map(|path| path.source))
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
      .flat_map(|path| vec![path.source, path.destination])
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
      Path {
        source: "a.txt",
        destination: "d.txt",
        create: true,
        exists: vec!["d.txt"],
      },
      Path {
        source: "b.txt",
        destination: "e.txt",
        create: true,
        exists: vec!["e.txt"],
      },
      Path {
        source: "c.txt",
        destination: "f.txt",
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
      Path {
        source: "a.txt",
        destination: "d.txt",
        create: true,
        exists: vec!["a.txt", "d.txt"],
      },
      Path {
        source: "b.txt",
        destination: "e.txt",
        create: true,
        exists: vec!["b.txt", "e.txt"],
      },
      Path {
        source: "c.txt",
        destination: "f.txt",
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
      Path {
        source: "a.txt",
        destination: "d.txt",
        create: true,
        exists: vec!["d.txt"],
      },
      Path {
        source: "b.txt",
        destination: "e.txt",
        create: true,
        exists: vec!["e.txt"],
      },
      Path {
        source: "c.txt",
        destination: "f.txt",
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
      Path {
        source: "a.txt",
        destination: "d.txt",
        create: true,
        exists: vec!["a.txt"],
      },
      Path {
        source: "b.txt",
        destination: "e.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Path {
        source: "c.txt",
        destination: "f.txt",
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
      Path {
        source: "a.txt",
        destination: "b.txt",
        create: false,
        exists: vec![],
      },
      Path {
        source: "c.txt",
        destination: "b.txt",
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
      Path {
        source: "a.txt",
        destination: "c.txt",
        create: true,
        exists: vec!["a.txt"],
      },
      Path {
        source: "b.txt",
        destination: "c.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Path {
        source: "e.txt",
        destination: "f.txt",
        create: true,
        exists: vec!["e.txt"],
      },
      Path {
        source: "e.txt",
        destination: "f.txt",
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
      Path {
        source: "a.txt",
        destination: "b.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Path {
        source: "b.txt",
        destination: "c.txt",
        create: true,
        exists: vec!["c.txt", "b.txt"],
      },
      Path {
        source: "d.txt",
        destination: "e.txt",
        create: true,
        exists: vec!["e.txt"],
      },
    ])
    .argument("--force")
    .argument("--temp")
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
      Path {
        source: "a.txt",
        destination: "a.txt",
        create: true,
        exists: vec!["a.txt"],
      },
      Path {
        source: "b.txt",
        destination: "b.txt",
        create: true,
        exists: vec!["b.txt"],
      },
      Path {
        source: "c.txt",
        destination: "c.txt",
        create: true,
        exists: vec!["c.txt"],
      },
    ])
    .argument("--force")
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
      Path {
        source: "a.txt",
        destination: "foo/a.txt",
        create: true,
        exists: vec!["a.txt"],
      },
      Path {
        source: "b.txt",
        destination: "bar/baz/c.txt",
        create: true,
        exists: vec!["b.txt"],
      },
    ])
    .expected_status(1)
    .expected_stderr(
      "
      error: Found destination(s) with non-existent directory(ies): foo/a.txt, bar/baz/c.txt
      ",
    )
    .run()
}

#[test]
fn circular_rename() -> Result {
  Test::new()?
    .sources(vec![
      Path {
        source: "a.txt",
        destination: "b.txt",
        create: true,
        exists: vec!["a.txt", "b.txt"],
      },
      Path {
        source: "b.txt",
        destination: "a.txt",
        create: true,
        exists: vec!["b.txt", "a.txt"],
      },
    ])
    .argument("--force")
    .argument("--temp")
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
