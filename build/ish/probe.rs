// Exercise the real target libatomic entry points required by openssl-sys.
#[link(name = "atomic", kind = "static")]
extern "C" {
    fn __atomic_is_lock_free(size: usize, ptr: *const u8) -> bool;
    fn __atomic_load_8(ptr: *const u64, order: i32) -> u64;
    fn __atomic_store_8(ptr: *mut u64, value: u64, order: i32);
    fn __atomic_fetch_add_8(ptr: *mut u64, value: u64, order: i32) -> u64;
}
#[repr(align(8))]
struct Aligned(u64);
fn main() {
    assert_eq!(std::mem::size_of::<usize>(), 4);
    assert_eq!(std::mem::size_of::<*const u8>(), 4);
    let mut value = Aligned(0);
    unsafe {
        assert!(__atomic_is_lock_free(4, (&value.0 as *const u64).cast()));
        assert!(!__atomic_is_lock_free(17, (&value.0 as *const u64).cast()));
        let _ = __atomic_is_lock_free(8, (&value.0 as *const u64).cast());
        __atomic_store_8(&mut value.0, 0x1234_5678_9abc_def0, 5);
        assert_eq!(__atomic_fetch_add_8(&mut value.0, 7, 5), 0x1234_5678_9abc_def0);
        assert_eq!(__atomic_load_8(&value.0, 5), 0x1234_5678_9abc_def7);
    }
    println!("i586-musl Pentium + target libatomic cross probe OK: UTF-8 café 🦀");
}
