use crate::Color;
use alloc::boxed::Box;
use core::mem::MaybeUninit;
use embedded_graphics;
use embedded_graphics::prelude::*;
use embedded_graphics::{drawable, DrawTarget};

pub struct DisplayDriver {
    pub(crate) raw: lvgl_sys::lv_disp_drv_t,
}

impl DisplayDriver {
    // we should accept a Rc<RefCell<T>> and throw it in a box and add to the user_data of the callback handler function
    pub fn new<T, C>(device: &mut T) -> Self
    where
        T: DrawTarget<C>,
        C: PixelColor + From<Color>,
    {
        let disp_drv = unsafe {
            // Create a display buffer for LittlevGL
            let mut display_buffer =
                Box::new(MaybeUninit::<lvgl_sys::lv_disp_buf_t>::uninit().assume_init());

            // Declare a buffer for the refresh rate
            const REFRESH_BUFFER_LEN: usize = 2;
            let refresh_buffer1 = Box::new(
                MaybeUninit::<
                    [MaybeUninit<lvgl_sys::lv_color_t>;
                        lvgl_sys::LV_HOR_RES_MAX as usize * REFRESH_BUFFER_LEN],
                >::uninit()
                .assume_init(),
            );
            let refresh_buffer2 = Box::new(
                MaybeUninit::<
                    [MaybeUninit<lvgl_sys::lv_color_t>;
                        lvgl_sys::LV_HOR_RES_MAX as usize * REFRESH_BUFFER_LEN],
                >::uninit()
                .assume_init(),
            );

            // Initialize the display buffer
            lvgl_sys::lv_disp_buf_init(
                display_buffer.as_mut(),
                Box::into_raw(refresh_buffer1) as *mut cty::c_void,
                Box::into_raw(refresh_buffer2) as *mut cty::c_void,
                lvgl_sys::LV_HOR_RES_MAX * REFRESH_BUFFER_LEN as u32,
            );

            // Descriptor of a display driver
            let mut disp_drv = MaybeUninit::<lvgl_sys::lv_disp_drv_t>::uninit().assume_init();

            // Basic initialization
            lvgl_sys::lv_disp_drv_init(&mut disp_drv);

            // Assign the buffer to the display
            disp_drv.buffer = Box::into_raw(display_buffer);

            // Set your driver function
            disp_drv.flush_cb = Some(display_callback_wrapper::<T, C>);

            // TODO: DrawHandler type here
            disp_drv.user_data = device as *mut _ as *mut cty::c_void;

            disp_drv
        };
        Self { raw: disp_drv }
    }
}

// We need to keep a reference to the DisplayDriver in UI if we implement Drop
// impl Drop for DisplayDriver {
//     fn drop(&mut self) {
//         // grab the user data and deref the DrawHandler to free the instance for dealloc in the Rust universe.
//         unimplemented!()
//     }
// }

// a reference is kept to the external drawing target (T)
// the reference is kept in the callback function of the drawing handler
// we need a reference counter for the drawing target and free the ref counter when the display is
// destroyed.
//type DrawHandler = Rc<RefCell<u8>>;
//
// impl Drop for DrawHandler {
//     fn drop(&mut self) {
//         unimplemented!()
//     }
// }

unsafe extern "C" fn display_callback_wrapper<T, C>(
    disp_drv: *mut lvgl_sys::lv_disp_drv_t,
    area: *const lvgl_sys::lv_area_t,
    color_p: *mut lvgl_sys::lv_color_t,
) where
    T: DrawTarget<C>,
    C: PixelColor + From<Color>,
{
    // We need to make sure panics can't escape across the FFI boundary.
    //let _ = std::panic::catch_unwind(|| {
    let display_driver = *disp_drv;

    // Rust code closure reference
    let device = &mut *(display_driver.user_data as *mut T);

    let ys = (*area).y1..=(*area).y2;
    let xs = ((*area).x1..=(*area).x2).enumerate();
    let x_len = ((*area).x2 - (*area).x1 + 1) as usize;
    let pixels = ys
        .enumerate()
        .map(|(iy, y)| {
            xs.clone().map(move |(ix, x)| {
                let color_len = x_len * iy + ix;
                let raw_color = Color::from_raw(*color_p.add(color_len));
                drawable::Pixel(Point::new(x as i32, y as i32), raw_color.into())
            })
        })
        .flatten();
    // TODO: Maybe find a way to use `draw_image` method on the device instance.
    let _ = device.draw_iter(pixels.into_iter());

    // Indicate to LittlevGL that you are ready with the flushing
    lvgl_sys::lv_disp_flush_ready(disp_drv);
    //}); // end of panic::catch_unwind
}
