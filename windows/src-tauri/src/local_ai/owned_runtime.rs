//! Own the server and its model runners together so closing MoMo releases VRAM.
use std::process::Child;

pub(super) struct OwnedRuntime {
    pub child: Child,
    #[cfg(windows)]
    job: Job,
}

impl OwnedRuntime {
    pub fn new(child: Child) -> Result<Self, String> {
        #[cfg(windows)]
        let job = match Job::attach(&child) {
            Ok(job) => job,
            Err(_) => {
                let mut child = child;
                let _ = child.kill();
                let _ = child.wait();
                return Err(
                    "Could not isolate the local AI runtime. Restart MoMo and try again.".into(),
                );
            }
        };
        Ok(Self {
            child,
            #[cfg(windows)]
            job,
        })
    }
}

impl Drop for OwnedRuntime {
    fn drop(&mut self) {
        #[cfg(windows)]
        self.job.close();
        #[cfg(target_os = "linux")]
        if self.child.try_wait().ok().flatten().is_none() {
            // This group was created solely for the server spawned by MoMo.
            unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) };
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(windows)]
struct Job(isize);

#[cfg(windows)]
impl Job {
    fn attach(child: &Child) -> windows::core::Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::JobObjects::*;
        // A non-inherited handle remains owned by MoMo. Windows also closes it
        // if MoMo crashes, terminating only this job's server and descendants.
        let handle = unsafe { CreateJobObjectW(None, windows::core::PCWSTR::null())? };
        let job = Self(handle.0 as isize);
        let limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                ..Default::default()
            },
            ..Default::default()
        };
        unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const std::ffi::c_void,
                std::mem::size_of_val(&limits) as u32,
            )?;
            AssignProcessToJobObject(handle, HANDLE(child.as_raw_handle()))?;
        }
        Ok(job)
    }

    fn close(&mut self) {
        if self.0 != 0 {
            let handle = windows::Win32::Foundation::HANDLE(self.0 as *mut std::ffi::c_void);
            self.0 = 0;
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
        }
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows::Win32::System::Threading::{
        OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
    };

    #[test]
    fn dropping_owned_runtime_terminates_its_model_runner() {
        let powershell = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let child = Command::new(powershell)
            .args(["-NoProfile", "-NonInteractive", "-Command",
                "[Console]::ReadLine() | Out-Null; $runner = Start-Process -FilePath ($env:SystemRoot + '\\System32\\WindowsPowerShell\\v1.0\\powershell.exe') -ArgumentList @('-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 30') -WindowStyle Hidden -PassThru; [Console]::WriteLine($runner.Id); Start-Sleep -Seconds 30"])
            .creation_flags(0x0800_0000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn().unwrap();
        let mut runtime = OwnedRuntime::new(child).unwrap();
        writeln!(runtime.child.stdin.take().unwrap(), "ready").unwrap();
        let mut line = String::new();
        BufReader::new(runtime.child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let runner_pid = line.trim().parse::<u32>().unwrap();
        let runner = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, runner_pid).unwrap() };
        drop(runtime);
        let result = unsafe { WaitForSingleObject(runner, 5000) };
        unsafe { CloseHandle(runner).unwrap() };
        assert_eq!(
            result, WAIT_OBJECT_0,
            "The model runner outlived its owned server"
        );
    }
}
