use libdivecomputer_sys as ffi;

/// Safe wrapper around `dc_buffer_t`.
pub struct Buffer {
    pub(crate) ptr: *mut ffi::dc_buffer_t,
}

impl Buffer {
    /// Create a new buffer with the given initial capacity.
    pub fn new(capacity: usize) -> Self {
        let ptr = unsafe { ffi::dc_buffer_new(capacity) };
        Self { ptr }
    }

    /// Get the buffer contents as a slice.
    pub fn as_slice(&self) -> &[u8] {
        if self.ptr.is_null() {
            return &[];
        }
        unsafe {
            let data = ffi::dc_buffer_get_data(self.ptr);
            let size = ffi::dc_buffer_get_size(self.ptr);
            if data.is_null() || size == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(data, size)
            }
        }
    }

    /// Copy the buffer contents into a Vec.
    pub fn to_vec(&self) -> Vec<u8> {
        self.as_slice().to_vec()
    }

    /// Get the buffer size.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        if self.ptr.is_null() {
            0
        } else {
            unsafe { ffi::dc_buffer_get_size(self.ptr) }
        }
    }

    /// Check if the buffer is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                ffi::dc_buffer_free(self.ptr);
            }
        }
    }
}
