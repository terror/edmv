use {
  executable_path::executable_path,
  pretty_assertions::assert_eq,
  std::{fs::File, process::Command, str},
  tempdir::TempDir,
  unindent::Unindent,
};

type Result<T = (), E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

struct Test {
  arguments: Vec<String>,
  expected_status: i32,
  expected_stderr: String,
  expected_stdout: String,
  paths: Vec<(String, String, bool)>,
  tempdir: TempDir,
}

impl Test {
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

  fn paths(self, paths: Vec<(&str, &str, bool)>) -> Self {
    Self {
      paths: paths
        .into_iter()
        .map(|(old, new, should_rename)| {
          (old.to_owned(), new.to_owned(), should_rename)
        })
        .collect(),
      ..self
    }
  }

  fn run(self) -> Result {
    self.run_and_return_tempdir().map(|_| ())
  }

  fn command(&self) -> Result<Command> {
    let mut command = Command::new(executable_path(env!("CARGO_PKG_NAME")));

    let old = self
      .paths
      .iter()
      .map(|path| path.0.clone())
      .collect::<Vec<_>>();

    for path in &old {
      File::create(self.tempdir.path().join(path))?;
    }

    let editor = format!(
      "echo -e '{}' >",
      self
        .paths
        .iter()
        .map(|path| path.1.clone())
        .collect::<Vec<_>>()
        .join("\n")
    );

    command
      .current_dir(&self.tempdir)
      .args(old)
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

    for (old, new, should_rename) in self.paths {
      if should_rename {
        assert!(!self.tempdir.path().join(old).exists());
        assert!(self.tempdir.path().join(new).exists());
      } else {
        assert!(self.tempdir.path().join(old).exists());
        assert!(!self.tempdir.path().join(new).exists());
      }
    }

    Ok(self.tempdir)
  }
}

#[test]
fn renames_non_existing_file_paths() -> Result {
  Test::new()?
    .paths(vec![
      ("a.txt", "d.txt", true),
      ("b.txt", "e.txt", true),
      ("c.txt", "f.txt", true),
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
      ("a.txt", "b.txt", false),
      ("b.txt", "e.txt", true),
      ("c.txt", "f.txt", true),
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
