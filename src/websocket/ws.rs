use wasm_bindgen::prelude::*;
use yew::services::ConsoleService;
use yew::Callback;

use std::rc::Rc;
use std::cell::RefCell;

use rustc_hash::FxHashMap;
use serde_json::Value;
use serde::{Serialize, Deserialize};
use crossbeam::queue::SegQueue;

use crate::websocket::bind;
use crate::websocket::identifiers::{
    Subscriber,
    WebsocketMessage,
    WebsocketStatus
};
use crate::opcodes::OpCode;


/// The internal websocket wrapped in a Rc and RefCell to make it
/// cheap to clone.
type InternalHandle = Rc<RefCell<InternalWebSocket>>;


/// The base message for all websocket messages, giving the op code
/// that is used to send the payload to their relevant events.
#[derive(Serialize, Deserialize)]
pub struct WrappingWsMessage {
    /// The opcode of the message.
    pub(crate) opcode: OpCode,

    /// The payload / data for the given opcode.
    pub(crate) payload: Option<Value>,
}


/// A cheaply cloneable handle for interacting with the websocket e.g.
/// adding and removing subscribers of a given id.
#[derive(Clone)]
pub struct WsHandler {
    internal: InternalHandle,
    status_queue: StatusUpdateQueue,
    message_queue: MessageUpdateQueue,
}

impl WsHandler {
    /// Connects to a given websocket returning a handle.
    pub fn connect(url: impl Into<String>) -> WsHandler {
        let (internal, status, message) = InternalWebSocket::connect(url.into());

        Self {
            internal,
            status_queue: status,
            message_queue: message,
        }
    }

    pub fn subscribe_to_status(
        &self,
        id: usize,
        cb: Callback<WebsocketStatus>
    ) {
        self.status_queue.push((id, cb));
    }

    pub fn subscribe_to_message(
        &self,
        id: usize,
        opcode: OpCode,
        cb: Callback<WebsocketMessage>,
    ) {
        self.message_queue.push((id, opcode, cb));
    }
}

type MessageUpdateQueue = Rc<SegQueue<(usize, OpCode, Callback<WebsocketMessage>)>>;
type StatusUpdateQueue = Rc<SegQueue<(usize, Callback<WebsocketStatus>)>>;


/// The internal Websocket handle that contains all the WASM interactions
/// in order to properly link the ws with its events.
pub struct InternalWebSocket {
    /// The websocket connection url
    url: String,

    /// The internal websocket value, used to keep it alive in the heap.
    internal: Option<JsValue>,

    /// Signals if the ws closed on us or we just arent conencted yet.
    connecting_first: bool,

    /// The amount of attempts to re-connect on a disconnect.
    retry_attempt: usize,

    /// The js callback for `onopen`.
    js_open: Option<Closure<dyn FnMut()>>,

    /// The js callback for `onclose`.
    js_close: Option<Closure<dyn FnMut()>>,

    /// The js callback for `onerror`.
    js_error: Option<Closure<dyn FnMut()>>,

    /// The js callback for `onmessage`.
    js_message: Option<Closure<dyn FnMut(String)>>,

    /// The subscribers of the websocket, subscribing to events.
    subscribers: FxHashMap<usize, Subscriber>,

    message_updates: MessageUpdateQueue,
    status_updates: StatusUpdateQueue,
}

impl InternalWebSocket {
    /// Connects to a given websocket.
    fn connect(url: String) -> (InternalHandle, StatusUpdateQueue, MessageUpdateQueue) {
        let status_update = Rc::new(SegQueue::new());
        let message_update = Rc::new(SegQueue::new());

        let ws = Rc::new(RefCell::new(InternalWebSocket {
            url: url.clone(),
            internal: None,
            retry_attempt: 0,
            connecting_first: true,

            js_open: None,
            js_close: None,
            js_error: None,
            js_message: None,

            subscribers: FxHashMap::default(),
            message_updates: message_update.clone(),
            status_updates: status_update.clone(),
        }));


        let on_open = Closure::wrap({
            let ws2 = ws.clone();
            Box::new(move || {
                ws2.borrow_mut().on_connect();
            }) as Box<dyn FnMut()>
        });

        let on_close = Closure::wrap({
            let ws2 = ws.clone();
            Box::new(move || {
                ws2.borrow_mut().on_disconnect();
            }) as Box<dyn FnMut()>
        });

        let on_error = Closure::wrap({
            let ws2 = ws.clone();
            Box::new(move || {
                ws2.borrow_mut().on_error();
            }) as Box<dyn FnMut()>
        });

        let on_message = Closure::wrap({
            let ws2 = ws.clone();
            Box::new(move |msg: String| {
                ws2.borrow_mut().on_message(msg);
            }) as Box<dyn FnMut(String)>
        });

        let socket = bind::start_websocket(
            url.clone(),
            &on_open,
            &on_close,
            &on_error,
            &on_message,
        );

        {
            let mut inst_mut = ws.borrow_mut();
            inst_mut.internal = Some(socket);
            inst_mut.js_open = Some(on_open);
            inst_mut.js_close = Some(on_close);
            inst_mut.js_error = Some(on_error);
            inst_mut.js_message = Some(on_message);
        }

        (ws, status_update, message_update)
    }

    /// The websocket has opened and is connected.
    fn on_connect(&mut self) {
        self.retry_attempt = 0;

        self.check_status_updates();
        self.send_all_status(WebsocketStatus::Connect);
    }

    /// The websocket is closed and has disconnected.
    fn on_disconnect(&mut self) {
        let status = if self.retry_attempt > 3 {
            WebsocketStatus::ClosedPermanently
        } else {
            self.retry_attempt += 1;
            self.reconnect();
            WebsocketStatus::Disconnect
        };

        self.check_status_updates();
        self.send_all_status(status);
    }

    /// An error has happened on the websocket.
    fn on_error(&mut self) {
        self.connecting_first = false;
    }

    /// A message has been received by the websocket.
    fn on_message(&mut self, msg: String) {
        let maybe_success = serde_json::from_str::<WrappingWsMessage>(&msg);
        let msg = if let Ok(msg) = maybe_success {
            msg
        } else {
            let msg = format!("Failed to parse incoming message! {:?}", &msg);
            ConsoleService::log(&msg);
            return;
        };

        let opcode = msg.opcode;
        let msg = if let Some(payload ) = msg.payload {
            WebsocketMessage::Payload(payload)
        } else {
            WebsocketMessage::Empty
        };

        self.check_message_updates();
        for (_, sub) in self.subscribers.iter() {
            sub.emit_message(opcode, msg.clone())
        }
    }

    /// Attempts to reconnect to the socket.
    fn reconnect(&mut self) {
        if self.connecting_first {
            return
        }

        let socket = bind::start_websocket(
            self.url.clone(),
            &self.js_open.as_ref().unwrap(),
            &self.js_close.as_ref().unwrap(),
            &self.js_error.as_ref().unwrap(),
            &self.js_message.as_ref().unwrap(),
        );

        self.internal = Some(socket);
    }

    fn check_status_updates(&mut self) {
        while let Some((id, cb)) = self.status_updates.pop() {
            if let Some(sub) = self.subscribers.get_mut(&id) {
                sub.set_status_cb(cb);
            } else {
                let mut sub = Subscriber::new();
                sub.set_status_cb(cb);
                self.subscribers.insert(id, sub);
            }
        }
    }

    fn check_message_updates(&mut self) {
        while let Some((id, opcode, cb)) = self.message_updates.pop() {
            if let Some(sub) = self.subscribers.get_mut(&id) {
                sub.subscribe(opcode, cb);
            } else {
                let mut sub = Subscriber::new();
                sub.subscribe(opcode, cb);
                self.subscribers.insert(id, sub);
            }
        }
    }

    /// Sends the status to all subscribers.
    fn send_all_status(&self, status: WebsocketStatus) {
        for (_, sub) in self.subscribers.iter() {
            sub.emit_status(status.clone());
        };
    }
}
