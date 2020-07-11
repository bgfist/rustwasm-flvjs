#![allow(non_snake_case)]

#[macro_use]
extern crate serde_derive;

use wasm_bindgen::prelude::*;

mod io;
mod core;
mod demux;
mod remux;
mod utils;
mod panic;
mod web_sys_wrappers;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(start)]
pub fn start() {
    panic::set_panic_hook();
}
