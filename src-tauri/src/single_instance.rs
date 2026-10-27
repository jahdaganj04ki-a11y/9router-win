// Single-Instance-Garantie via benanntes Windows-Mutex.
// Bei zweiter Instanz: bestehendes Fenster in den Vordergrund holen, dann exit.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Threading::{CreateMutexW, ReleaseMutex};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE};

const MUTEX_NAME: &str = "Local\\9routerSingleInstanceMutex";
const WINDOW_TITLE: &str = "9router";

pub struct SingleInstance {
    #[cfg(target_os = "windows")]
    handle: *mut std::ffi::c_void,
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn bring_existing_to_front() {
    let wide = to_wide(WINDOW_TITLE);
    unsafe {
        let hwnd = FindWindowW(ptr::null(), wide.as_ptr());
        if hwnd != std::ptr::null_mut() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

impl SingleInstance {
    pub fn acquire() -> Option<Self> {
        #[cfg(target_os = "windows")]
        {
            let name = to_wide(MUTEX_NAME);
            unsafe {
                let handle = CreateMutexW(ptr::null_mut(), 0, name.as_ptr());
                if handle.is_null() {
                    return None;
                }
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    CloseHandle(handle);
                    bring_existing_to_front();
                    return None;
                }
                Some(Self { handle })
            }
        }
        #[cfg(not(target_os = "windows"))]
        Some(Self { handle: ptr::null_mut() })
    }
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        unsafe {
            if !self.handle.is_null() {
                let _ = ReleaseMutex(self.handle);
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
