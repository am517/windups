//! How Walter reaches other windups. Windups call windups; nothing else schedules work.
use std::path::PathBuf;
use std::process::Command;

pub trait Caller {
    /// The windup's profile (`<cmd> --capabilities`), if it answers.
    fn capabilities(&self, cmd: &[String]) -> Option<String>;
    /// Runs the windup on these files; returns its report.
    fn call(&self, cmd: &[String], files: &[PathBuf]) -> Result<String, String>;
}

/// Runs windups as programs.
pub struct Process;

impl Caller for Process {
    fn capabilities(&self, cmd: &[String]) -> Option<String> {
        let (p, a) = cmd.split_first()?;
        let o = Command::new(p)
            .args(a)
            .args(["--capabilities", "--windup"])
            .output()
            .ok()?;
        o.status
            .success()
            .then(|| String::from_utf8_lossy(&o.stdout).into_owned())
    }
    fn call(&self, cmd: &[String], files: &[PathBuf]) -> Result<String, String> {
        let (p, a) = cmd.split_first().ok_or("empty command")?;
        let o = Command::new(p)
            .args(a)
            .args(files)
            .output()
            .map_err(|e| format!("{p}: {e}"))?;
        Ok(String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
    }
}
