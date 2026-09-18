use crate::files;
use anyhow::{Context, Result, bail};
use std::{
    ffi::OsStr,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn run_input(cwd: &Path, args: &[&str], input: &[u8]) -> Result<String> {
    let mut child = command("git")
        .args(["--no-optional-locks", "-c", "protocol.ext.allow=never"])
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Git could not be started")?;
    let mut stdin = child.stdin.take().context("Missing Git input pipe")?;
    let output = std::thread::scope(|scope| -> Result<_> {
        let writer = scope.spawn(move || stdin.write_all(input));
        let output = child.wait_with_output()?;
        writer
            .join()
            .map_err(|_| anyhow::anyhow!("Git input writer failed"))??;
        Ok(output)
    })?;
    if !output.status.success() {
        bail!(
            "Git command failed: {}",
            redact(&String::from_utf8_lossy(&output.stderr)).trim()
        );
    }
    String::from_utf8(output.stdout).context("Git returned text that is not valid UTF-8")
}

pub fn command(program: impl AsRef<OsStr>) -> Command {
    let cmd = Command::new(program);
    #[cfg(windows)]
    let cmd = {
        use std::os::windows::process::CommandExt;
        let mut cmd = cmd;
        cmd.creation_flags(0x08000000);
        cmd
    };
    cmd
}

pub fn run<I, S>(cwd: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = command("git")
        .args(["--no-optional-locks", "-c", "protocol.ext.allow=never"])
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::inherit())
        .output()
        .context("Git could not be started. Install Git and check your PATH")?;
    if !output.status.success() {
        let detail = redact(&String::from_utf8_lossy(&output.stderr));
        bail!("Git command failed: {}", detail.trim());
    }
    Ok(String::from_utf8(output.stdout)
        .context("Git returned text that is not valid UTF-8")?
        .trim()
        .to_owned())
}

fn redact(input: &str) -> String {
    let mut result = input.to_owned();
    for scheme in ["https://", "http://"] {
        let mut offset = 0;
        while let Some(index) = result[offset..].find(scheme) {
            let start = offset + index + scheme.len();
            let length = result[start..]
                .find(|c: char| c.is_whitespace() || c == '/')
                .unwrap_or(result.len() - start);
            if let Some(at) = result[start..start + length].find('@') {
                result.replace_range(start..start + at, "[redacted]");
                offset = start + "[redacted]@".len();
            } else {
                offset = start;
            }
        }
    }
    result
}

pub fn root(path: &Path) -> Result<PathBuf> {
    files::directory(path)?;
    let actual = files::absolute(run(path, ["rev-parse", "--show-toplevel"])?)?;
    // Canonicalization handles case and junctions in a user's enclosing projects path.
    let expected = std::fs::canonicalize(path)?;
    let actual_canonical = std::fs::canonicalize(&actual)?;
    if !files::same(&expected, &actual_canonical) {
        bail!("Use the repository root: {}", actual.display());
    }
    files::absolute(path)
}

pub fn enclosing(path: &Path) -> Option<PathBuf> {
    run(path, ["rev-parse", "--show-toplevel"])
        .ok()
        .and_then(|p| files::absolute(p).ok())
}

pub fn clone(source: &str, target: &Path) -> Result<()> {
    let cwd = target.parent().context("Clone destination has no parent")?;
    run(
        cwd,
        [
            OsStr::new("clone"),
            OsStr::new("--no-recurse-submodules"),
            OsStr::new("--no-hardlinks"),
            OsStr::new("--"),
            OsStr::new(source),
            target.as_os_str(),
        ],
    )?;
    Ok(())
}
