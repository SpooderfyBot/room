use yew::utils::document;
use yew::{Component, ComponentLink};
use std::future::Future;
use serde::Serialize;
use reqwest::Client;
use wasm_bindgen_futures::spawn_local;

use crate::settings;


/// Extracts the room id from the expecting url.
pub fn get_room_id() -> String {
    let doc = document();
    let mut url = doc.url().unwrap();

    let split_at = settings::get_room_url().len();
    let room_id = url.split_off(split_at + 1);

    room_id
}

/// Starts a future with a completion callback of a given component link.
pub fn send_future<COMP: Component, F>(link: ComponentLink<COMP>, future: F)
where
    F: Future<Output = COMP::Message> + 'static,
{
    spawn_local(async move {
        link.send_message(future.await);
    });
}


/// Starts a future which returns nothing.
pub fn start_future<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    spawn_local(future);
}


pub async fn emit_event<T: Serialize>(room_id: String, payload: T) {
    let url = settings::get_emit_url(&room_id);

    let _ = Client::new()
        .put(&url)
        .json(&payload)
        .send()
        .await;
}

