use wasm_bindgen::prelude::*;

// wasm-bindgen will automatically take care of including this script
#[wasm_bindgen(module = "/src/webtorrent/js/wt.js")]
extern "C" {
    #[wasm_bindgen(js_name = "extractFiles")]
    pub fn extract_files(torrent_id: &str, callback: JsValue);

    #[wasm_bindgen(js_name = "extractTitle")]
    pub fn extract_title(file: &JsValue) -> String;

    #[wasm_bindgen(js_name = "sendToVideoInN")]
    pub fn render_to_video(elm_id: &str, file: &JsValue, n: u32);
}