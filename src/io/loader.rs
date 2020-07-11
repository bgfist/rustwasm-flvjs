use js_sys::Function;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    pub type Loader;

    #[wasm_bindgen(method, setter, js_name = "onDataArrival")]
    fn set_onDataArrival(this: &Loader, val: Function);
}
