//! Opt-in diagnostics for isolated verification processes, never other apps.
use std::{path::PathBuf, sync::OnceLock};
use windows_sys::Win32::System::{Diagnostics::Debug::*, LibraryLoader::*, Threading::*};
static REPORT: OnceLock<PathBuf> = OnceLock::new();
pub fn trace_verification_crashes(path: PathBuf) {
    let _ = REPORT.set(path);
    // SAFETY: process-lifetime callback/storage, installed only by the explicit
    // desktop verification entry point. No handler or state in another process.
    unsafe {
        SetUnhandledExceptionFilter(Some(crashed));
    }
}
unsafe extern "system" fn crashed(info: *const EXCEPTION_POINTERS) -> i32 {
    if let Some(path) = REPORT.get()
        && !info.is_null()
    {
        // SAFETY: Windows lends exception/context records during this callback.
        // Stack bytes are copied with ReadProcessMemory, never dereferenced.
        unsafe {
            let info = &*info;
            if !info.ExceptionRecord.is_null() && !info.ContextRecord.is_null() {
                let exception = &*info.ExceptionRecord;
                let context = &*info.ContextRecord;
                let mut report = format!(
                    "exception={:#x} rip={:#x} rsp={:#x} thread={}\n",
                    exception.ExceptionCode,
                    context.Rip,
                    context.Rsp,
                    GetCurrentThreadId()
                );
                for index in 0..48 {
                    let mut address = 0usize;
                    let mut read = 0usize;
                    if ReadProcessMemory(
                        GetCurrentProcess(),
                        (context.Rsp as usize + index * 8) as *const _,
                        (&mut address as *mut usize).cast(),
                        8,
                        &mut read,
                    ) == 0
                        || read != 8
                    {
                        break;
                    }
                    let mut module = 0;
                    if GetModuleHandleExW(
                        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
                        address as *const u16,
                        &mut module,
                    ) != 0
                    {
                        let mut name = [0u16; 2048];
                        let length =
                            GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32);
                        report.push_str(&format!(
                            "stack[{index}]={address:#x} {} +{:#x}\n",
                            String::from_utf16_lossy(&name[..length as usize]),
                            address - module as usize
                        ));
                        // The crashing process is about to exit; keep module
                        // references alive through diagnostic symbol resolution.
                    }
                }
                let _ = std::fs::write(path, report);
            }
        }
    }
    1 // EXCEPTION_EXECUTE_HANDLER: terminate this failed verification process.
}
