use wasm_bindgen::prelude::*;

// wasm-bindgen will automatically take care of including this script
#[wasm_bindgen(module = "/src/websocket/js/handle_ws.js")]
extern "C" {
    #[wasm_bindgen(js_name = "startWs")]
    pub fn start_websocket(
        url: String,
        on_open: &Closure<dyn FnMut()>,
        on_close: &Closure<dyn FnMut()>,
        on_error: &Closure<dyn FnMut()>,
        on_message: &Closure<dyn FnMut(String)>,
    ) -> JsValue;
}