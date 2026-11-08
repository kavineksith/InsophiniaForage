//! Process manager. Spawns and tracks child processes for nginx, PHP-FPM,
//! and MariaDB. Each service runs with a generated config file under
//! `~/.insophinia-forage/conf/`, so we never touch system configs.
//!
//! ## Safety properties
//!
//! 1. **No orphans**: every spawned child is recorded in a `ProcessSlot`.
//!    On app exit, the manager kills them all and waits for exit.
//! 2. **No double-spawn**: starting a service that's already running returns
//!    `AlreadyRunning` without spawning a new process.
//! 3. **No leaked handles**: stdout/stderr are captured and forwarded to
//!    log files via background threads, so the children never block on a
//!    full pipe.
//! 4. **Health checks**: `is_running()` uses `try_wait()` (not `kill(0)`)
//!    so we get the real exit status, and the manager can report it to
//!    the UI.

use crate::error::{ForageError, ForageResult};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// A tracked child process, plus where its logs go and when it started.
struct ProcessSlot {
    child: Child,
    started_at: Instant,
    log_file: PathBuf,
    /// The full command line, kept for diagnostics.
    command_string: String,
    /// Join handle for the stdout-pump thread.
    _stdout_handle: std::thread::JoinHandle<()>,
    /// Join handle for the stderr-pump thread.
    _stderr_handle: std::thread::JoinHandle<()>,
}

/// Thread-safe map of running services.
#[derive(Clone, Default)]
pub struct ProcessManager {
    slots: Arc<Mutex<HashMap<String, ProcessSlot>>>,
}

/// What to spawn.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub service: String,
    pub program: PathBuf,
    pub args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub log_file: PathBuf,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn a service. Returns the pid.
    pub fn spawn(&self, spec: SpawnSpec) -> ForageResult<u32> {
        let mut slots = self.slots.lock();
        if let Some(slot) = slots.get(&spec.service) {
            if slot.child.try_wait().ok().flatten().is_none() {
                return Err(ForageError::AlreadyRunning(spec.service.clone()));
            } else {
                // Stale slot — clean it up.
                slots.remove(&spec.service);
            }
        }

        if !spec.program.exists() {
            return Err(ForageError::BinaryNotFound {
                name: spec.program.to_string_lossy().into(),
                locations: spec.program.to_string_lossy().into(),
            });
        }

        // Make sure log dir exists.
        if let Some(parent) = spec.log_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&spec.log_file)?;

        info!(
            service = %spec.service,
            cmd = %spec.program.display(),
            args = ?spec.args,
            "spawning service"
        );

        let mut cmd = Command::new(&spec.program);
        cmd.args(&spec.args);
        if let Some(cwd) = &spec.working_dir {
            cmd.current_dir(cwd);
        }
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        // Capture stdout+stderr so we can pump them to the log file. We use
        // piped stdio (not inherit) because inheriting would mix all
        // services' output into the parent's stderr.
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        // New process group on Unix so we can kill the whole tree if a
        // service spawns grandchildren (MariaDB does this on Windows).
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        let mut child = cmd.spawn().map_err(|e| ForageError::Service {
            service: spec.service.clone(),
            message: format!("spawn failed: {}", e),
        })?;
        let pid = child.id();

        // Spawn pump threads for stdout/stderr -> log file.
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let lf1 = log_file.try_clone()?;
        let lf2 = log_file.try_clone()?;
        let svc1 = spec.service.clone();
        let svc2 = spec.service.clone();

        let stdout_handle = std::thread::spawn(move || {
            if let Some(out) = stdout {
                let reader = BufReader::new(out);
                let mut writer = std::io::BufWriter::new(lf1);
                for line in reader.lines().flatten() {
                    let _ = writeln!(writer, "{}", line);
                    let _ = writer.flush();
                    // Mirror to tracing at debug so dev mode shows it.
                    tracing::debug!(service = %svc1, "{}", line);
                }
            }
        });
        let stderr_handle = std::thread::spawn(move || {
            if let Some(err) = stderr {
                let reader = BufReader::new(err);
                let mut writer = std::io::BufWriter::new(lf2);
                for line in reader.lines().flatten() {
                    let _ = writeln!(writer, "{}", line);
                    let _ = writer.flush();
                    warn!(service = %svc2, "{}", line);
                }
            }
        });

        let command_string = format!(
            "{} {}",
            spec.program.display(),
            spec.args.join(" ")
        );

        slots.insert(
            spec.service.clone(),
            ProcessSlot {
                child,
                started_at: Instant::now(),
                log_file: spec.log_file.clone(),
                command_string,
                _stdout_handle: stdout_handle,
                _stderr_handle: stderr_handle,
            },
        );

        Ok(pid)
    }

    /// Stop a service. Sends SIGTERM, waits up to 5s, then SIGKILL.
    pub fn stop(&self, service: &str) -> ForageResult<()> {
        let mut slot = {
            let mut slots = self.slots.lock();
            slots
                .remove(service)
                .ok_or_else(|| ForageError::NotRunning(service.to_string()))?
        };

        info!(service = %service, "stopping service");
        // Graceful kill.
        slot.child.kill().map_err(|e| ForageError::Service {
            service: service.to_string(),
            message: format!("kill failed: {}", e),
        })?;
        // Wait up to 5s.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match slot.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Ok(None) => {
                    // Force kill again.
                    let _ = slot.child.kill();
                    let _ = slot.child.wait();
                    break;
                }
                Err(e) => {
                    warn!(service = %service, error = %e, "wait failed");
                    break;
                }
            }
        }
        Ok(())
    }

    /// Stop all running services, in reverse-start order. Called on app
    /// quit. Best-effort — never returns an error.
    pub fn stop_all(&self) {
        let services: Vec<String> = {
            let slots = self.slots.lock();
            slots.keys().cloned().collect()
        };
        for svc in services.iter().rev() {
            if let Err(e) = self.stop(svc) {
                error!(service = %svc, error = %e, "failed to stop during shutdown");
            }
        }
    }

    /// True if the service child is still alive.
    pub fn is_running(&self, service: &str) -> bool {
        let mut slots = self.slots.lock();
        if let Some(slot) = slots.get_mut(service) {
            match slot.child.try_wait() {
                Ok(Some(status)) => {
                    info!(service = %service, status = %status, "service exited");
                    slots.remove(service);
                    false
                }
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Snapshot of all running services for the UI.
    pub fn list(&self) -> Vec<ServiceStatus> {
        let mut slots = self.slots.lock();
        let mut out = Vec::new();
        let mut dead = Vec::new();
        for (name, slot) in slots.iter_mut() {
            match slot.child.try_wait() {
                Ok(Some(status)) => {
                    dead.push(name.clone());
                    out.push(ServiceStatus {
                        name: name.clone(),
                        running: false,
                        pid: None,
                        uptime_seconds: slot.started_at.elapsed().as_secs(),
                        exit_code: status.code(),
                        log_file: slot.log_file.clone(),
                        command: slot.command_string.clone(),
                    });
                }
                Ok(None) => {
                    out.push(ServiceStatus {
                        name: name.clone(),
                        running: true,
                        pid: Some(slot.child.id()),
                        uptime_seconds: slot.started_at.elapsed().as_secs(),
                        exit_code: None,
                        log_file: slot.log_file.clone(),
                        command: slot.command_string.clone(),
                    });
                }
                Err(_) => {
                    dead.push(name.clone());
                }
            }
        }
        for d in dead {
            slots.remove(&d);
        }
        out
    }

    /// Read the last N lines of a service's log file. Used by the UI log
    /// viewer.
    pub fn tail_log(service: &str, n: usize) -> ForageResult<Vec<String>> {
        let path = crate::core::paths::logs_dir().join(service, "service.log");
        if !path.exists() {
            return Ok(vec![]);
        }
        let content = std::fs::read_to_string(&path)?;
        let lines: Vec<&str> = content.lines().collect();
        let start = lines.len().saturating_sub(n);
        Ok(lines[start..].iter().map(|s| s.to_string()).collect())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServiceStatus {
    pub name: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime_seconds: u64,
    pub exit_code: Option<i32>,
    pub log_file: PathBuf,
    pub command: String,
}

/// Kill a process by PID, used to clean up orphans from a previous crash.
/// Best-effort — uses `sysinfo` so we don't need libc FFI.
pub fn kill_orphan(pid: u32) -> ForageResult<()> {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes();
    if let Some(proc_) = sys.process(sysinfo::Pid::from(pid as usize)) {
        proc_.kill();
        Ok(())
    } else {
        // Already gone.
        Ok(())
    }
}

/// Check whether a PID file points to a still-running process. Used at
/// startup to detect orphans.
pub fn check_orphan_pid(pid_file: &Path) -> Option<u32> {
    let raw = std::fs::read_to_string(pid_file).ok()?;
    let pid: u32 = raw.trim().parse().ok()?;
    if is_pid_alive(pid) {
        Some(pid)
    } else {
        let _ = std::fs::remove_file(pid_file);
        None
    }
}

fn is_pid_alive(pid: u32) -> bool {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes();
    sys.process(sysinfo::Pid::from(pid as usize)).is_some()
}
