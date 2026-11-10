// Selector de carpetas nativo de Windows (sin dependencias extra).
use std::ffi::c_void;
use std::os::windows::ffi::OsStringExt;

#[repr(C)]
struct BrowseInfoW {
    hwnd_owner: isize,
    pidl_root: *const c_void,
    display_name: *mut u16,
    title: *const u16,
    flags: u32,
    callback: Option<unsafe extern "system" fn(isize, u32, isize, isize) -> i32>,
    l_param: isize,
    image: i32,
}

#[link(name = "shell32")]
extern "system" {
    fn SHBrowseForFolderW(info: *mut BrowseInfoW) -> *mut c_void;
    fn SHGetPathFromIDListW(pidl: *const c_void, path: *mut u16) -> i32;
}

#[link(name = "ole32")]
extern "system" {
    fn CoTaskMemFree(pv: *const c_void);
    fn CoInitializeEx(reserved: *const c_void, coinit: u32) -> i32;
    fn CoUninitialize();
}

#[link(name = "user32")]
extern "system" {
    fn GetActiveWindow() -> isize;
}

const BIF_RETURNONLYFSDIRS: u32 = 0x0001;
const BIF_EDITBOX: u32 = 0x0010;
const BIF_NEWDIALOGSTYLE: u32 = 0x0040;
const COINIT_APARTMENTTHREADED: u32 = 0x2;
const MAX_PATH: usize = 260;

pub fn pick_folder(title: &str) -> Option<String> {
    unsafe {
        let hr = CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED);
        let needs_uninit = hr >= 0;

        let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let mut display = vec![0u16; MAX_PATH];
        let mut info = BrowseInfoW {
            hwnd_owner: GetActiveWindow(),
            pidl_root: std::ptr::null(),
            display_name: display.as_mut_ptr(),
            title: title_wide.as_ptr(),
            flags: BIF_RETURNONLYFSDIRS | BIF_EDITBOX | BIF_NEWDIALOGSTYLE,
            callback: None,
            l_param: 0,
            image: 0,
        };

        let pidl = SHBrowseForFolderW(&mut info);
        if pidl.is_null() {
            if needs_uninit {
                CoUninitialize();
            }
            return None;
        }

        let mut path = vec![0u16; MAX_PATH];
        let ok = SHGetPathFromIDListW(pidl, path.as_mut_ptr());
        CoTaskMemFree(pidl);
        if needs_uninit {
            CoUninitialize();
        }
        if ok == 0 {
            return None;
        }

        let len = path.iter().position(|&c| c == 0).unwrap_or(0);
        let wide = &path[..len];
        Some(
            std::ffi::OsString::from_wide(wide)
                .to_string_lossy()
                .to_string(),
        )
    }
}
