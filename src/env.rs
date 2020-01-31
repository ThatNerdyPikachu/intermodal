use crate::common::*;

pub(crate) struct Env {
  args: Vec<String>,
  dir: Box<dyn AsRef<Path>>,
  pub(crate) err: Box<dyn Write>,
  pub(crate) out: Box<dyn Write>,
  err_style: Style,
}

impl Env {
  pub(crate) fn main() -> Self {
    let dir = match env::current_dir() {
      Ok(dir) => dir,
      Err(error) => panic!("Failed to get current directory: {}", error),
    };

    let err_style = if env::var_os("NO_COLOR").is_some()
      || env::var_os("TERM").as_deref() == Some(OsStr::new("dumb"))
      || !atty::is(atty::Stream::Stderr)
    {
      Style::inactive()
    } else {
      Style::active()
    };

    Self::new(dir, io::stdout(), io::stderr(), err_style, env::args())
  }

  pub(crate) fn run(&mut self) -> Result<(), Error> {
    #[cfg(windows)]
    ansi_term::enable_ansi_support().ok();

    #[cfg(not(test))]
    env_logger::Builder::from_env(
      env_logger::Env::new()
        .filter("JUST_LOG")
        .write_style("JUST_LOG_STYLE"),
    )
    .init();

    let opt = Opt::from_iter_safe(&self.args)?;

    match opt.use_color {
      UseColor::Always => self.err_style = Style::active(),
      UseColor::Auto => {}
      UseColor::Never => self.err_style = Style::inactive(),
    }

    opt.run(self)
  }

  pub(crate) fn new<D, O, E, S, I>(dir: D, out: O, err: E, err_style: Style, args: I) -> Self
  where
    D: AsRef<Path> + 'static,
    O: Write + 'static,
    E: Write + 'static,
    S: Into<String>,
    I: IntoIterator<Item = S>,
  {
    Self {
      args: args.into_iter().map(Into::into).collect(),
      dir: Box::new(dir),
      err: Box::new(err),
      out: Box::new(out),
      err_style,
    }
  }

  pub(crate) fn status(&mut self) -> Result<(), i32> {
    use structopt::clap::ErrorKind;

    if let Err(error) = self.run() {
      if let Error::Clap { source } = error {
        if source.use_stderr() {
          write!(&mut self.err, "{}", source).ok();
        } else {
          write!(&mut self.out, "{}", source).ok();
        }
        match source.kind {
          ErrorKind::VersionDisplayed | ErrorKind::HelpDisplayed => Ok(()),
          _ => Err(EXIT_FAILURE),
        }
      } else {
        writeln!(
          &mut self.err,
          "{}{}: {}{}",
          self.err_style.error().paint("error"),
          self.err_style.message().prefix(),
          error,
          self.err_style.message().suffix(),
        )
        .ok();
        Err(EXIT_FAILURE)
      }
    } else {
      Ok(())
    }
  }

  pub(crate) fn dir(&self) -> &Path {
    self.dir.as_ref().as_ref()
  }

  pub(crate) fn resolve(&self, path: impl AsRef<Path>) -> PathBuf {
    self.dir().join(path).clean()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn error_message_on_stdout() {
    let mut env = testing::env(
      [
        "torrent",
        "create",
        "--input",
        "foo",
        "--announce",
        "udp:bar.com",
        "--announce-tier",
        "foo",
      ]
      .iter()
      .cloned(),
    );
    fs::write(env.resolve("foo"), "").unwrap();
    env.status().ok();
    let err = env.err();
    if !err.starts_with("error: Failed to parse announce URL:") {
      panic!("Unexpected standard error output: {}", err);
    }

    assert_eq!(env.out(), "");
  }
}