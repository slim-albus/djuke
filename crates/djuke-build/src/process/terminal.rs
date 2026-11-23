use std::io::{self, Write};
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

#[cfg(windows)]
mod win32 {
    use std::os::raw::c_void;

    pub type Handle = *mut c_void;
    pub type Dword = u32;
    pub type Bool = i32;

    pub const STD_INPUT_HANDLE: Dword = (-10i32) as Dword;
    pub const STD_OUTPUT_HANDLE: Dword = (-11i32) as Dword;
    pub const STD_ERROR_HANDLE: Dword = (-12i32) as Dword;

    pub const CTRL_C_EVENT: Dword = 0;
    pub const CTRL_BREAK_EVENT: Dword = 1;

    extern "system" {
        pub fn GetStdHandle(nStdHandle: Dword) -> Handle;
        pub fn GetConsoleMode(hConsoleHandle: Handle, lpMode: *mut Dword) -> Bool;
        pub fn SetConsoleMode(hConsoleHandle: Handle, dwMode: Dword) -> Bool;
        pub fn SetConsoleCtrlHandler(
            HandlerRoutine: Option<unsafe extern "system" fn(Dword) -> Bool>,
            Add: Bool,
        ) -> Bool;
    }
}

#[cfg(windows)]
static HAS_SAVED_MODES: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static SAVED_IN_MODE: AtomicU32 = AtomicU32::new(0);
#[cfg(windows)]
static SAVED_OUT_MODE: AtomicU32 = AtomicU32::new(0);
#[cfg(windows)]
static SAVED_ERR_MODE: AtomicU32 = AtomicU32::new(0);
#[cfg(windows)]
static HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
unsafe extern "system" fn console_ctrl_handler(ctrl_type: win32::Dword) -> win32::Bool {
    if ctrl_type == win32::CTRL_C_EVENT || ctrl_type == win32::CTRL_BREAK_EVENT {
        restore_terminal_state();
    }
    0 // Return 0 (FALSE) so the default signal handling or termination proceeds cleanly
}

/// Initialize terminal safety handler. Installs console ctrl handler on Windows.
pub fn init_terminal_safety() {
    #[cfg(windows)]
    {
        capture_current_console_modes();
        if !HANDLER_INSTALLED.swap(true, Ordering::SeqCst) {
            unsafe {
                win32::SetConsoleCtrlHandler(Some(console_ctrl_handler), 1);
            }
        }
    }
}

/// Capture current console modes so they can be restored later.
pub fn capture_current_console_modes() {
    #[cfg(windows)]
    {
        unsafe {
            let h_in = win32::GetStdHandle(win32::STD_INPUT_HANDLE);
            let mut in_mode: win32::Dword = 0;
            if win32::GetConsoleMode(h_in, &mut in_mode) != 0 {
                SAVED_IN_MODE.store(in_mode, Ordering::SeqCst);
            }

            let h_out = win32::GetStdHandle(win32::STD_OUTPUT_HANDLE);
            let mut out_mode: win32::Dword = 0;
            if win32::GetConsoleMode(h_out, &mut out_mode) != 0 {
                SAVED_OUT_MODE.store(out_mode, Ordering::SeqCst);
            }

            let h_err = win32::GetStdHandle(win32::STD_ERROR_HANDLE);
            let mut err_mode: win32::Dword = 0;
            if win32::GetConsoleMode(h_err, &mut err_mode) != 0 {
                SAVED_ERR_MODE.store(err_mode, Ordering::SeqCst);
            }

            HAS_SAVED_MODES.store(true, Ordering::SeqCst);
        }
    }
}

/// Restores the console modes to their captured state and resets terminal attributes & cursor.
pub fn restore_terminal_state() {
    #[cfg(windows)]
    {
        if HAS_SAVED_MODES.load(Ordering::SeqCst) {
            unsafe {
                let in_mode = SAVED_IN_MODE.load(Ordering::SeqCst);
                if in_mode != 0 {
                    let h_in = win32::GetStdHandle(win32::STD_INPUT_HANDLE);
                    win32::SetConsoleMode(h_in, in_mode);
                }

                let out_mode = SAVED_OUT_MODE.load(Ordering::SeqCst);
                if out_mode != 0 {
                    let h_out = win32::GetStdHandle(win32::STD_OUTPUT_HANDLE);
                    win32::SetConsoleMode(h_out, out_mode);
                }

                let err_mode = SAVED_ERR_MODE.load(Ordering::SeqCst);
                if err_mode != 0 {
                    let h_err = win32::GetStdHandle(win32::STD_ERROR_HANDLE);
                    win32::SetConsoleMode(h_err, err_mode);
                }
            }
        }
    }

    // Always reset cursor visibility and text attributes to standard
    // \x1b[?25h: show cursor
    // \x1b[0m: reset all attributes
    let _ = write!(io::stdout(), "\x1b[?25h\x1b[0m");
    let _ = io::stdout().flush();
}

/// RAII Guard that ensures terminal mode is restored when dropped.
#[derive(Debug, Default)]
pub struct TerminalGuard;

impl TerminalGuard {
    pub fn acquire() -> Self {
        capture_current_console_modes();
        init_terminal_safety();
        Self
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal_state();
    }
}
