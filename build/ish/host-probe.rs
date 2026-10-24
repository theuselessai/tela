use std::ffi::{c_char, c_int, c_void, CStr, CString};
extern "C" {
    fn dlopen(path: *const c_char, flags: c_int) -> *mut c_void;
    fn dlerror() -> *const c_char;
    fn dlclose(handle: *mut c_void) -> c_int;
}
fn main() {
    assert_eq!(std::mem::size_of::<usize>(), 8);
    let path = CString::new("/usr/lib/llvm21/lib/libclang.so.21.1.2").unwrap();
    unsafe {
        let handle = dlopen(path.as_ptr(), 2);
        if handle.is_null() {
            panic!("native libclang dlopen failed: {:?}", CStr::from_ptr(dlerror()));
        }
        assert_eq!(dlclose(handle), 0);
    }
    println!("native ARM64 dynamically linked Rust can load libclang");
}
