use std::slice;

use crate::Decoder;

#[unsafe(no_mangle)]
pub extern "C" fn qoi_decode(
    data: *const u8,
    len: usize,
    out_width: *mut usize,
    out_height: *mut usize,
) -> *const u8 {
    if data.is_null() {
        return std::ptr::null();
    }

    let mut decoder = Decoder::new();
    let decoded = decoder.decode(unsafe { slice::from_raw_parts(data, len) });

    let Some(image) = decoded else {
        return std::ptr::null();
    };

    if !out_width.is_null() {
        unsafe { *out_width = image.width };
    }
    if !out_height.is_null() {
        unsafe { *out_height = image.height };
    }

    let bytes = transmute_vec::<_, u8>(image.pixels);
    let boxed = Box::new(bytes).into_boxed_slice();

    Box::into_raw(boxed).cast()
}

#[unsafe(no_mangle)]
pub extern "C" fn qoi_free(data: *mut u8) {
    if !data.is_null() {
        drop(unsafe { Box::from_raw(data) });
    }
}

fn transmute_vec<Src, Dst>(input: Vec<Src>) -> Vec<Dst> {
    const fn get_factor(src_size: usize, dst_size: usize) -> usize {
        if src_size == 0 && dst_size == 0 {
            1
        } else {
            assert!(dst_size != 0 && src_size % dst_size == 0);
            src_size / dst_size
        }
    }

    let factor = const { get_factor(size_of::<Src>(), size_of::<Dst>()) };

    let len = input.len() * factor;
    let cap = input.capacity() * factor;

    unsafe { Vec::from_raw_parts(input.leak().as_mut_ptr().cast(), len, cap) }
}
