use wasm_bindgen::prelude::*;

// wasm-bindgen will automatically take care of including this script
#[wasm_bindgen(module = "/src/js/player.js")]
extern "C" {
    #[wasm_bindgen(js_name = "setPlayerListeners")]
    pub fn set_listeners(on_error: &Closure<dyn FnMut()>, on_meta: &Closure<dyn FnMut()>) -> bool;

    #[wasm_bindgen(js_name = "tryReloadVideo")]
    pub fn try_reload();
}