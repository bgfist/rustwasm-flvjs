use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    pub type Log;

    #[wasm_bindgen(static_method_of = Log)]
    fn e(tag: &str, msg: &str);

    #[wasm_bindgen(static_method_of = Log)]
    fn w(tag: &str, msg: &str);

    #[wasm_bindgen(static_method_of = Log)]
    fn d(tag: &str, msg: &str);

    #[wasm_bindgen(static_method_of = Log)]
    fn v(tag: &str, msg: &str);
}
