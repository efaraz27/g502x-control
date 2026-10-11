//! Launch resident/UI processes without inheriting caller pipes or device handles.
use anyhow::{Context, Result};

pub fn detached(args: &[&str]) -> Result<()> {
    let executable = std::env::current_exe()?;
    #[cfg(not(windows))]
    {
        use std::{
            os::unix::process::CommandExt,
            process::{Command, Stdio},
        };
        let mut child = Command::new(executable)
            .args(args)
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Starting application process")?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
    #[cfg(windows)]
    {
        use std::{os::windows::ffi::OsStrExt, ptr};
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::Threading::{
                CREATE_NO_WINDOW, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
            },
        };
        let path: Vec<u16> = executable
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // All supported arguments are fixed internal switches. Reject anything
        // that would need Windows command-line escaping before constructing it.
        anyhow::ensure!(
            args.iter()
                .all(|a| matches!(*a, "--background" | "--settings")),
            "Unsupported launch argument"
        );
        let mut command = vec![b'"' as u16];
        command.extend(executable.as_os_str().encode_wide());
        command.push(b'"' as u16);
        for arg in args {
            command.push(b' ' as u16);
            command.extend(arg.encode_utf16());
        }
        command.push(0);
        // Zeroed native structs are valid with cb set; no handles are inherited.
        let mut startup: STARTUPINFOW = unsafe { std::mem::zeroed() };
        startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        let mut process: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        // std::process::Command on Windows can inherit unrelated inheritable
        // handles, including a caller's stdout pipe, even with Stdio::null().
        // Explicit FALSE keeps a resident from keeping CLI/installer pipes open.
        let ok = unsafe {
            CreateProcessW(
                path.as_ptr(),
                command.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                CREATE_NO_WINDOW,
                ptr::null(),
                ptr::null(),
                &startup,
                &mut process,
            )
        };
        if ok == 0 {
            return Err(std::io::Error::last_os_error()).context("Starting application process");
        }
        // Closing these handles does not terminate the detached child.
        unsafe {
            CloseHandle(process.hThread);
            CloseHandle(process.hProcess);
        }
        Ok(())
    }
}
