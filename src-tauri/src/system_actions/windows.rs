use ::windows::{
    core::{w, Interface, Result, PCSTR, PCWSTR},
    Win32::{
        Foundation::{
            GetLastError, ERROR_CLASS_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, POINT,
            WPARAM,
        },
        System::{
            Com::CoTaskMemFree,
            LibraryLoader::GetModuleHandleW,
            Ole::{OleFlushClipboard, OleInitialize, OleUninitialize},
        },
        UI::{
            Shell::{
                Common::ITEMIDLIST, IContextMenu, IContextMenu2, IContextMenu3, IShellFolder,
                SHBindToParent, SHParseDisplayName, CMF_NORMAL, CMIC_MASK_PTINVOKE,
                CMINVOKECOMMANDINFO, CMINVOKECOMMANDINFOEX, SEE_MASK_NOASYNC, SEE_MASK_UNICODE,
            },
            WindowsAndMessaging::*,
        },
    },
};
use std::{cell::RefCell, path::Path};

thread_local! {
    static HANDLER: RefCell<Option<(Option<IContextMenu2>, Option<IContextMenu3>)>> = const { RefCell::new(None) };
}

struct Ole;
impl Ole {
    fn new() -> Result<Self> {
        // The helper's main thread owns all COM objects until they are released.
        unsafe {
            OleInitialize(None)?;
        }
        Ok(Self)
    }
}
impl Drop for Ole {
    fn drop(&mut self) {
        unsafe {
            OleUninitialize();
        }
    }
}

struct Pidl(*mut ITEMIDLIST);
impl Drop for Pidl {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(Some(self.0.cast()));
        }
    }
}
struct Menu(HMENU);
impl Drop for Menu {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyMenu(self.0);
        }
    }
}
struct Window(HWND);
impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if matches!(
        message,
        WM_INITMENUPOPUP | WM_DRAWITEM | WM_MEASUREITEM | WM_MENUCHAR
    ) {
        let handled = HANDLER.with(|handler| {
            let handler = handler.borrow();
            let (menu2, menu3) = handler.as_ref()?;
            if let Some(menu) = menu3 {
                let mut result = LRESULT::default();
                if menu
                    .HandleMenuMsg2(message, wparam, lparam, Some(&mut result))
                    .is_ok()
                {
                    return Some(result);
                }
            }
            if let Some(menu) = menu2 {
                if message != WM_MENUCHAR && menu.HandleMenuMsg(message, wparam, lparam).is_ok() {
                    return Some(LRESULT(0));
                }
            }
            None
        });
        if let Some(result) = handled {
            return result;
        }
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}

fn window() -> Result<Window> {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: w!("myCmdSystemMenu"),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
            return Err(::windows::core::Error::from_win32());
        }
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.lpszClassName,
            w!("myCmd system actions"),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(instance),
            None,
        )
        .map(Window)
    }
}

fn parsing_name(path: &Path) -> String {
    let path = path.to_string_lossy();
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        path.strip_prefix(r"\\?\").unwrap_or(&path).to_owned()
    }
}

fn context_menu(path: &Path, hwnd: HWND) -> Result<(IContextMenu, Menu, Pidl)> {
    // Canonical filesystem paths use extended prefixes that Explorer does not accept.
    let path: Vec<u16> = parsing_name(path).encode_utf16().chain(Some(0)).collect();
    unsafe {
        let mut raw = std::ptr::null_mut();
        SHParseDisplayName(PCWSTR(path.as_ptr()), None, &mut raw, 0, None)?;
        let pidl = Pidl(raw);
        let mut child = std::ptr::null_mut();
        let folder: IShellFolder = SHBindToParent(pidl.0, Some(&mut child))?;
        let context: IContextMenu = folder.GetUIObjectOf(hwnd, &[child], None)?;
        let menu = Menu(CreatePopupMenu()?);
        context
            .QueryContextMenu(menu.0, 0, 1, 0x7fff, CMF_NORMAL)
            .ok()?;
        Ok((context, menu, pidl))
    }
}

pub fn show(path: &Path) -> Result<()> {
    let _ole = Ole::new()?;
    let window = window()?;
    let (context, menu, _pidl) = context_menu(path, window.0)?;
    HANDLER
        .with(|handler| *handler.borrow_mut() = Some((context.cast().ok(), context.cast().ok())));
    // All extension callbacks and command execution stay in this helper process.
    let result = unsafe {
        let run = || -> Result<()> {
            let mut point = POINT::default();
            GetCursorPos(&mut point)?;
            let _ = ShowWindow(window.0, SW_SHOWNOACTIVATE);
            let _ = SetForegroundWindow(window.0);
            let command = TrackPopupMenuEx(
                menu.0,
                (TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
                point.x,
                point.y,
                window.0,
                None,
            )
            .0;
            let _ = PostMessageW(Some(window.0), WM_NULL, WPARAM(0), LPARAM(0));
            if command != 0 {
                let ordinal = (command - 1) as usize;
                let invoke = CMINVOKECOMMANDINFOEX {
                    cbSize: std::mem::size_of::<CMINVOKECOMMANDINFOEX>() as u32,
                    fMask: SEE_MASK_UNICODE | SEE_MASK_NOASYNC | CMIC_MASK_PTINVOKE,
                    hwnd: window.0,
                    lpVerb: PCSTR(ordinal as *const u8),
                    lpVerbW: PCWSTR(ordinal as *const u16),
                    nShow: SW_SHOWNORMAL.0,
                    ptInvoke: point,
                    ..Default::default()
                };
                context.InvokeCommand(
                    (&invoke as *const CMINVOKECOMMANDINFOEX).cast::<CMINVOKECOMMANDINFO>(),
                )?;
                // Keep Copy/Cut data usable after this isolated COM host exits.
                let _ = OleFlushClipboard();
            }
            Ok(())
        };
        run()
    };
    HANDLER.with(|handler| *handler.borrow_mut() = None);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converts_canonical_paths_to_shell_parsing_names() {
        assert_eq!(
            parsing_name(Path::new(r"\\?\C:\Data\日本語.txt")),
            r"C:\Data\日本語.txt"
        );
        assert_eq!(
            parsing_name(Path::new(r"\\?\UNC\server\share\file.txt")),
            r"\\server\share\file.txt"
        );
        assert_eq!(
            parsing_name(Path::new(r"C:\Data\file.txt")),
            r"C:\Data\file.txt"
        );
    }
    #[test]
    fn shell_builds_a_real_context_menu_for_a_local_file() {
        let _ole = Ole::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("native menu & 日本語.txt");
        std::fs::write(&path, "keep").unwrap();
        let window = window().unwrap();
        let (_context, menu, _pidl) = context_menu(&path, window.0).unwrap();
        assert!(unsafe { GetMenuItemCount(Some(menu.0)) } > 0);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "keep");
    }
}
