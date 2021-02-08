#![allow(unused)]

use serde_json::{Value, Error};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use yew::Callback;
use rustc_hash::FxHashMap;

use crate::opcodes::OpCode;


/// Represents the state of the Websocket for listeners
/// to update their context and display messages.
#[derive(Clone)]
pub enum WebsocketStatus {
    /// Websocket has opened and is connect.
    Connect,

    /// Websocket has disconnected, the handle will attempt to reconnect.
    Disconnect,

    /// The websocket has disconnected and has exceeded the retry limit causing
    /// the handler to abort attempts and permanently disconnected.
    ClosedPermanently,
}


/// A websocket message from a given OpCode.
#[derive(Debug, Clone)]
pub enum WebsocketMessage {
    /// There is no payload in this message.
    Empty,

    /// There is a payload contained in this message.
    Payload(Value),
}

impl WebsocketMessage {
    /// Consumes the payload value returning it's converted value.
    /// Panics is the value is not able to be serialized,
    /// and returns None if it is not a Payload type enum.
    pub fn unwrap_and_into<T: DeserializeOwned>(self) -> Option<T> {
        if let Self::Payload(value) = self {
            Some(serde_json::from_value::<T>(value).unwrap())
        } else {
            None
        }
    }
}


/// A subscriber, they can have both a status callback and a set of
/// message callbacks that link to the relevant opcode and callback pair.
pub struct Subscriber {
    on_ws_status: Option<Callback<WebsocketStatus>>,
    on_ws_message: FxHashMap<OpCode, Callback<WebsocketMessage>>,
}

impl Subscriber {
    /// Creates a new subscriber with no callbacks set.
    pub fn new() -> Self {
        Self {
            on_ws_status: None,
            on_ws_message: FxHashMap::default(),
        }
    }

    /// Registers the given callback to be invoked upon a status change.
    pub fn set_status_cb(&mut self, cb: Callback<WebsocketStatus>) {
        self.on_ws_status = Some(cb);
    }

    /// Emits a status event if the subscriber has a set status event.
    pub fn emit_status(&self, status: WebsocketStatus) {
        if let Some(cb) = self.on_ws_status.as_ref() {
            cb.emit(status);
        }
    }

    /// Subscribes to a given opcode to receive events on the given callback.
    pub fn subscribe(&mut self, opcode: OpCode, cb: Callback<WebsocketMessage>) {
        self.on_ws_message.insert(opcode, cb);
    }

    /// Emits a message with a given opcode if the opcode is registered to a
    /// callback.
    pub fn emit_message(&self, opcode: OpCode, msg: WebsocketMessage) {
        if let Some(cb) = self.on_ws_message.get(&opcode) {
            cb.emit(msg);
        }
    }
}