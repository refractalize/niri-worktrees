use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct CmdOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub trait CommandRunner {
    fn run(&self, program: &str, args: &[String], cwd: Option<&Path>) -> std::io::Result<CmdOutput>;
    fn spawn(&self, program: &str, args: &[String], cwd: Option<&Path>) -> std::io::Result<()>;
}

#[derive(Debug, Default)]
pub struct RealCommandRunner;

impl CommandRunner for RealCommandRunner {
    fn run(&self, program: &str, args: &[String], cwd: Option<&Path>) -> std::io::Result<CmdOutput> {
        let mut cmd = Command::new(program);
        cmd.args(args.iter().map(OsStr::new));
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        let output = cmd.output()?;
        Ok(CmdOutput {
            status: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    fn spawn(&self, program: &str, args: &[String], cwd: Option<&Path>) -> std::io::Result<()> {
        let mut cmd = Command::new(program);
        cmd.args(args.iter().map(OsStr::new));
        if let Some(cwd) = cwd {
            cmd.current_dir(cwd);
        }
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        cmd.spawn()?;
        Ok(())
    }
}

pub fn split_program(argv: &[String]) -> Option<(&str, Vec<String>)> {
    argv.split_first()
        .map(|(program, args)| (program.as_str(), args.to_vec()))
}

