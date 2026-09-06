//! Resolve and run the commands that restore a hibernated project's
//! dependencies. Commands are always shown to the user before they run.

use crate::model::{Project, Stack};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WakeStep {
    pub program: String,
    pub args: Vec<String>,
    /// The command as the user should read it, e.g. `pnpm install --frozen-lockfile`.
    pub display: String,
}

impl WakeStep {
    pub fn new(program: &str, args: &[&str]) -> Self {
        let mut display = program.to_string();
        for a in args {
            display.push(' ');
            display.push_str(a);
        }
        WakeStep {
            program: program.to_string(),
            args: args.iter().map(|a| a.to_string()).collect(),
            display,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WakePlan {
    pub project_id: String,
    pub cwd: PathBuf,
    pub package_manager: Option<String>,
    pub steps: Vec<WakeStep>,
    pub notes: Vec<String>,
}

/// The install command for a Node package manager, honouring the lockfile.
pub fn node_install_step(pm: &str) -> WakeStep {
    match pm {
        "pnpm" => WakeStep::new("pnpm", &["install", "--frozen-lockfile"]),
        "yarn" => WakeStep::new("yarn", &["install", "--immutable"]),
        "bun" => WakeStep::new("bun", &["install", "--frozen-lockfile"]),
        _ => WakeStep::new("npm", &["ci"]),
    }
}

pub fn python_steps(project: &Path, pm: Option<&str>) -> Vec<WakeStep> {
    match pm {
        Some("uv") => vec![WakeStep::new("uv", &["sync"])],
        Some("poetry") => vec![WakeStep::new("poetry", &["install"])],
        Some("pipenv") => vec![WakeStep::new("pipenv", &["install", "--dev"])],
        Some("pdm") => vec![WakeStep::new("pdm", &["install"])],
        _ => {
            let mut steps = Vec::new();
            let venv = project.join(".venv");
            if !venv.exists() {
                steps.push(WakeStep::new(PYTHON, &["-m", "venv", ".venv"]));
            }
            if project.join("requirements.txt").exists() {
                steps.push(WakeStep::new(
                    VENV_PYTHON,
                    &["-m", "pip", "install", "-r", "requirements.txt"],
                ));
            } else if project.join("pyproject.toml").exists() {
                steps.push(WakeStep::new(
                    VENV_PYTHON,
                    &["-m", "pip", "install", "-e", "."],
                ));
            }
            steps
        }
    }
}

#[cfg(windows)]
const PYTHON: &str = "python";
#[cfg(not(windows))]
const PYTHON: &str = "python3";
#[cfg(windows)]
const VENV_PYTHON: &str = ".venv\\Scripts\\python.exe";
#[cfg(not(windows))]
const VENV_PYTHON: &str = ".venv/bin/python";

/// Build the wake plan for a project, one step per detected ecosystem.
pub fn plan_for(project: &Project) -> Option<WakePlan> {
    let mut steps = Vec::new();
    let mut notes = Vec::new();
    let pm = project.package_manager.as_deref();
    let path = &project.path;

    for stack in &project.stacks {
        match stack {
            Stack::Node => steps.push(node_install_step(pm.unwrap_or("npm"))),
            Stack::Rust => steps.push(WakeStep::new("cargo", &["build"])),
            Stack::Python => steps.extend(python_steps(path, pm)),
            Stack::Go => steps.push(WakeStep::new("go", &["mod", "download"])),
            Stack::Maven => steps.push(WakeStep::new("mvn", &["-q", "package", "-DskipTests"])),
            Stack::Gradle => {
                let wrapper = if cfg!(windows) {
                    "gradlew.bat"
                } else {
                    "./gradlew"
                };
                if path.join(wrapper.trim_start_matches("./")).exists() {
                    steps.push(WakeStep::new(wrapper, &["build", "-x", "test"]));
                } else {
                    steps.push(WakeStep::new("gradle", &["build", "-x", "test"]));
                }
            }
            Stack::DotNet => steps.push(WakeStep::new("dotnet", &["restore"])),
            Stack::CocoaPods => steps.push(WakeStep::new("pod", &["install"])),
            Stack::Dart => {
                if project.frameworks.iter().any(|f| f == "Flutter") {
                    steps.push(WakeStep::new("flutter", &["pub", "get"]));
                } else {
                    steps.push(WakeStep::new("dart", &["pub", "get"]));
                }
            }
            Stack::Ruby => steps.push(WakeStep::new("bundle", &["install"])),
            Stack::Php => steps.push(WakeStep::new("composer", &["install"])),
        }
    }

    if steps.is_empty() {
        return None;
    }
    if project.stacks.contains(&Stack::Python) && matches!(pm, None | Some("pip")) {
        notes
            .push("A fresh virtual environment is created in .venv/ if one does not exist.".into());
    }
    notes.push("Commands run inside the project folder and nothing else.".into());

    Some(WakePlan {
        project_id: project.id.clone(),
        cwd: path.clone(),
        package_manager: project.package_manager.clone(),
        steps,
        notes,
    })
}

/// Run one step, streaming combined stdout/stderr lines to `on_line`.
/// Returns the process exit code (or -1 if terminated by a signal).
pub fn run_step(
    step: &WakeStep,
    cwd: &Path,
    cancel: &AtomicBool,
    on_line: &mut dyn FnMut(String),
) -> io::Result<i32> {
    let mut cmd = command_for(step);
    cmd.current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Keep installers from prompting.
    cmd.env("CI", "1");
    let mut child = cmd.spawn()?;

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || pump(out, tx)));
    }
    if let Some(err) = child.stderr.take() {
        let tx = tx.clone();
        readers.push(std::thread::spawn(move || pump(err, tx)));
    }
    drop(tx);

    let child = Arc::new(std::sync::Mutex::new(child));
    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(line) => on_line(line),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if cancel.load(Ordering::Relaxed) {
                    let _ = child.lock().unwrap().kill();
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    for r in readers {
        let _ = r.join();
    }
    let status = child.lock().unwrap().wait()?;
    Ok(status.code().unwrap_or(-1))
}

fn pump<R: io::Read>(reader: R, tx: std::sync::mpsc::Sender<String>) {
    let reader = BufReader::new(reader);
    for line in reader.lines().map_while(Result::ok) {
        if tx.send(line).is_err() {
            break;
        }
    }
}

fn command_for(step: &WakeStep) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // npm, pnpm, yarn and friends are .cmd shims on Windows.
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(&step.display);
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        cmd
    }
    #[cfg(not(windows))]
    {
        let mut cmd = Command::new(&step.program);
        cmd.args(&step.args);
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_steps_follow_lockfile() {
        assert_eq!(
            node_install_step("pnpm").display,
            "pnpm install --frozen-lockfile"
        );
        assert_eq!(
            node_install_step("yarn").display,
            "yarn install --immutable"
        );
        assert_eq!(
            node_install_step("bun").display,
            "bun install --frozen-lockfile"
        );
        assert_eq!(node_install_step("npm").display, "npm ci");
        assert_eq!(node_install_step("unknown").display, "npm ci");
    }

    #[test]
    fn python_steps_depend_on_manager() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("requirements.txt"), "").unwrap();
        let steps = python_steps(tmp.path(), Some("pip"));
        assert_eq!(steps.len(), 2);
        assert!(steps[0].display.contains("-m venv .venv"));
        assert!(steps[1].display.contains("pip install -r requirements.txt"));
        assert_eq!(
            python_steps(tmp.path(), Some("poetry"))[0].display,
            "poetry install"
        );
    }

    #[cfg(unix)]
    #[test]
    fn runs_a_step_and_streams_output() {
        let step = WakeStep::new("sh", &["-c", "echo hello; echo world 1>&2"]);
        let mut lines = Vec::new();
        let cancel = AtomicBool::new(false);
        let code = run_step(&step, Path::new("/"), &cancel, &mut |l| lines.push(l)).unwrap();
        assert_eq!(code, 0);
        lines.sort();
        assert_eq!(lines, vec!["hello", "world"]);
    }
}
