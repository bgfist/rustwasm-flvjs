use wasm_bindgen::prelude::*;
use js_sys::Object;

#[wasm_bindgen]
extern "C" {
    static Browser: Object; 
}