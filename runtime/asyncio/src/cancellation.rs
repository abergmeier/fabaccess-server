use std::ptr;
use std::any::Any;
use std::ffi::CString;
use ptr_meta::DynMetadata;

/// Cancellation callback to clean up I/O resources
///
/// This allows IO actions to properly cancel and have their resources cleaned up without having to
/// worry about the current state of the io_uring queues.
pub struct Cancellation {
    data: *mut (),
    metadata: usize,
    drop: unsafe fn (*mut (), usize),
}

pub unsafe trait Cancel {
    fn into_raw(self) -> (*mut (), usize);
    unsafe fn drop_raw(ptr: *mut (), metadata: usize);
}

pub unsafe trait CancelNarrow {
    fn into_narrow_raw(self) -> *mut ();
    unsafe fn drop_narrow_raw(ptr: *mut ());
}

unsafe impl<T: CancelNarrow> Cancel for T {
    fn into_raw(self) -> (*mut (), usize) {
        (T::into_narrow_raw(self), 0)
    }

    unsafe fn drop_raw(ptr: *mut (), _: usize) {
        T::drop_narrow_raw(ptr)
    }
}

unsafe impl<T> CancelNarrow for Box<T> {
    fn into_narrow_raw(self) -> *mut () {
        Box::into_raw(self) as *mut ()
    }

    unsafe fn drop_narrow_raw(ptr: *mut ()) {
        drop(Box::from_raw(ptr))
    }
}

unsafe impl<T> Cancel for Box<[T]> {
    fn into_raw(self) -> (*mut (), usize) {
        let len = self.len();
        (Box::into_raw(self) as *mut (), len)
    }

    unsafe fn drop_raw(ptr: *mut (), metadata: usize) {
        drop(Vec::from_raw_parts(ptr, metadata, metadata))
    }
}

// Cancel impl for panics
unsafe impl Cancel for Box<dyn Any + Send + Sync> {
    fn into_raw(self) -> (*mut (), usize) {
        let ptr = Box::into_raw(self);
        let metadata = ptr_meta::metadata(ptr as *mut dyn Any);
        let metadata = unsafe {
            // SAFETY: None. I happen to know that metadata is always exactly `usize`-sized for this
            // type but only `std` can guarantee it.
            std::mem::transmute::<_, usize>(metadata)
        };
        (ptr as *mut(), metadata)
    }

    unsafe fn drop_raw(ptr: *mut (), metadata: usize) {
        let boxed: Box<dyn Any> = unsafe {
            let metadata =
            // SAFETY: We did it the other way around so this is safe if the previous step was.
            std::mem::transmute::<_, DynMetadata<dyn Any>>(metadata);

            // We can then (safely) construct a fat pointer from the metadata and data address
            let ptr = ptr_meta::from_raw_parts_mut(ptr, metadata);

            // SAFETY: We know the pointer is valid since `Self::into_raw` took ownership and the
            // vtable was extracted from this known good reference.
            Box::from_raw(ptr)
        };
        drop(boxed)
    }
}

unsafe impl CancelNarrow for CString {
    fn into_narrow_raw(self) -> *mut () {
        self.into_raw() as *mut ()
    }

    unsafe fn drop_narrow_raw(ptr: *mut ()) {
        drop(CString::from_raw(ptr as *mut libc::c_char));
    }
}

unsafe impl CancelNarrow for () {
    fn into_narrow_raw(self) -> *mut () {
        ptr::null_mut()
    }

    unsafe fn drop_narrow_raw(_: *mut ()) {}
}

unsafe impl<T, F> Cancel for (T, F)
    where T: CancelNarrow,
          F: CancelNarrow,
{
    fn into_raw(self) -> (*mut (), usize) {
        let (t, f) = self;
        let (t, _) = t.into_raw();
        let (f, _) = f.into_raw();
        (t, f as usize)
    }

    unsafe fn drop_raw(t: *mut (), f: usize) {
        T::drop_raw(t, 0);
        F::drop_raw(f as *mut (), 0);
    }
}

impl Cancellation {
    pub fn new<T: Cancel>(cancel: T) -> Self {
        let (data, metadata) = cancel.into_raw();
        Self { data, metadata, drop: T::drop_raw }
    }
}

impl Drop for Cancellation {
    fn drop(&mut self) {
        unsafe {
            (self.drop)(self.data, self.metadata)
        }
    }
}

impl<T: Cancel> From<T> for Cancellation {
    fn from(cancel: T) -> Self {
        Cancellation::new(cancel)
    }
}

impl<T> From<Option<T>> for Cancellation
    where Cancellation: From<T>
{
    fn from(option: Option<T>) -> Self {
        option.map_or(Cancellation::new(()), Cancellation::from)
    }
}
