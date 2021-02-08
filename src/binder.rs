use wasm_bindgen::prelude::*;

// wasm-bindgen will automatically take care of including this script
#[wasm_bindgen(module = "/src/js/player.js")]
extern "C" {
    #[wasm_bindgen(js_name = "play_video")]
    pub fn play_video(elm_id: &str);

    #[wasm_bindgen(js_name = "pause_video")]
    pub fn pause_video(elm_id: &str);

    #[wasm_bindgen(js_name = "seek_video")]
    pub fn seek_video(elm_id: &str, position: u32);

    #[wasm_bindgen(js_name = "mute_video")]
    pub fn mute_video(elm_id: &str);

    #[wasm_bindgen(js_name = "unmute_video")]
    pub fn unmute_video(elm_id: &str);

    #[wasm_bindgen(js_name = "get_pct_done")]
    pub fn get_pct_done(elm_id: &str) -> f32;

    #[wasm_bindgen(js_name = "get_duration")]
    pub fn get_duration(elm_id: &str) -> u32;

    #[wasm_bindgen(js_name = "get_pos")]
    pub fn get_pos(elm_id: &str) -> u32;

    #[wasm_bindgen(js_name = "set_vol")]
    pub fn set_vol(elm_id: &str, vol_pct: f32);

    #[wasm_bindgen(js_name = "maximise")]
    pub fn maximise(elm_id: &str);

    #[wasm_bindgen(js_name = "minimise")]
    pub fn minimise(elm_id: &str);
}