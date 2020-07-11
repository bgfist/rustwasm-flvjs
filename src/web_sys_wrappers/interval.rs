use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;

pub struct Interval {
    interval_id: i32,
    _closure: Closure<dyn FnMut()>,
}

impl Interval {
    pub fn new(closure: Box<dyn FnMut()>, timeout: i32) -> Interval {
        let window = web_sys::window().unwrap();

        let closure = Closure::wrap(closure);

        Interval {
            interval_id: window
                .set_interval_with_callback_and_timeout_and_arguments_0(
                    closure.as_ref().unchecked_ref(),
                    timeout,
                )
                .unwrap(),
            _closure: closure,
        }
    }
}

impl Drop for Interval {
    fn drop(&mut self) {
        let window = web_sys::window().unwrap();
        window.clear_interval_with_handle(self.interval_id);
    }
}
