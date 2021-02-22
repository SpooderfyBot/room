#![allow(unused)]

pub const SCHEMA: &str = "https";
pub const DOMAIN: &str = "spooderfy.com";
pub const GATEWAY_DOMAIN: &str = "gateway.spooderfy.com";
pub const WS_PATH: &str = "/ws";
pub const API_PATH: &str = "/api";

pub const EVENT_DISPLAY_ID: usize = 0;
pub const CHAT_ID: usize = 1;
pub const PLAYER_ID: usize = 2;


pub fn get_emit_url(room_id: &str) -> String {
    format!("{}://{}/emit/{}", SCHEMA, GATEWAY_DOMAIN, room_id)
}

pub fn get_ws_url(room_id: &str) -> String {
    format!("wss://{}{}/{}", GATEWAY_DOMAIN, WS_PATH, room_id)
}

pub fn get_webhook_api(room_id: &str) -> String {
    format!("{}://{}{}/room/{}/webhook", SCHEMA, DOMAIN, API_PATH, room_id)
}

pub fn get_stream_api_url(room_id: &str) -> String {
    format!("{}://{}{}/room/{}/stream", SCHEMA, DOMAIN, API_PATH, room_id)
}

pub fn get_who_am_i_url() -> String {
    format!("{}://{}{}/@me", SCHEMA, DOMAIN, API_PATH)
}

pub fn get_room_url() -> String {
    format!("{}://{}/room", SCHEMA, DOMAIN)
}