#![recursion_limit="512"]

mod binder;
mod player;
mod chat;
mod video;
mod opcodes;
mod websocket;
mod settings;
mod utils;
mod webtorrent;

use wasm_bindgen::prelude::*;
use yew::prelude::*;
use yew::services::TimeoutService;
use yew::services::timeout::TimeoutTask;

use std::time::Duration;
use crossbeam::queue::SegQueue;

use crate::websocket::{WsHandler, WebsocketStatus};


struct MovieRoom {
    ws: websocket::WsHandler,
    room_id: String,
}

impl Component for MovieRoom {
    type Message = ();
    type Properties = ();

    fn create(_props: Self::Properties, _link: ComponentLink<Self>) -> Self {
        let url = format!("ws://{}{}", settings::DOMAIN, settings::WS_PATH);
        let ws = WsHandler::connect(url);
        let room_id = utils::get_room_id();

        Self {
            ws,
            room_id,
        }
    }

    fn update(&mut self, _msg: Self::Message) -> ShouldRender {
        true
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        // Should only return "true" if new properties are different to
        // previously received properties.
        // This component has no properties so we will always return "false".
        false
    }

    fn view(&self) -> Html {
        html! {
            <div class="flex justify-around p-8">
                <player::MediaPlayer ws=self.ws.clone() room_id=self.room_id.clone() />

                <chat::ChatRoom ws=self.ws.clone() room_id=self.room_id.clone() />

                <WsEventDisplay ws=self.ws.clone() />
            </div>
        }
    }
}


#[derive(Properties, Clone)]
pub struct WsDisplayProperties {
    pub ws: WsHandler,
}

/// The events that can be invoked by callbacks
/// for the WsEventDisplay.
enum WsEventMessages {
    /// A websocket status update.
    Status(WebsocketStatus),

    /// A callback to hide the message.
    Hide,
}


/// Displays any websocket status events to the user.
///
/// This component renders the sticky elements that display the current state
/// of the websocket, it also manages hiding the messages are x seconds and
/// their timeouts.
struct WsEventDisplay {
    link: ComponentLink<Self>,
    _ws: WsHandler,

    hide: bool,
    pending_tasks: SegQueue<TimeoutTask>,

    connected: bool,
    connecting: bool,
    connection_dead: bool,
}

impl Component for WsEventDisplay {
    type Message = WsEventMessages;
    type Properties = WsDisplayProperties;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let ws = props.ws;

        let cb = link.callback(
            |event| WsEventMessages::Status(event)
        );
        ws.subscribe_to_status(settings::EVENT_DISPLAY_ID, cb);

        Self {
            link,

            _ws: ws,

            hide: false,
            pending_tasks: SegQueue::new(),

            connected: false,
            connecting: true,
            connection_dead: false
        }
    }

    fn update(&mut self, msg: Self::Message) -> bool {
        if let WsEventMessages::Status(status) = msg {
            match status {
                WebsocketStatus::Connect => {
                    self.connecting = false;
                    self.connected = true;
                    self.connection_dead = false;
                },
                WebsocketStatus::Disconnect => {
                    self.connecting = true;
                    self.connected = false;
                    self.connection_dead = false;
                },
                WebsocketStatus::ClosedPermanently => {
                    self.connecting = false;
                    self.connected = false;
                    self.connection_dead = true;
                },
            };
            self.hide = false;

            return true;
        }

        while let Some(_) = self.pending_tasks.pop() {
            continue
        }

        if self.connected {
            self.hide = true;
        }

        true
    }

    fn change(&mut self, _props: Self::Properties) -> bool {
        false
    }

    fn view(&self) -> Html {
        if self.hide {
            return html!{};
        }

        const BORDER_STYLE: &str = "border-gray-200 border-t-2 border-l-2 border-r-2 rounded-t-lg";
        const POSITION_CENTER: &str = "flex justify-around items-center";


        let bg_colour = if self.connecting {
            "bg-yellow-500"
        } else if self.connected {
            "bg-green-500"
        } else {
            "bg-red-500"
        };

        let div_style = format!(
            "{} {} {} py-2 px-4 w-2/3 ",
            bg_colour,
            BORDER_STYLE,
            POSITION_CENTER,
        );

        let msg = if self.connecting {
            "Connecting to servers..."
        } else if self.connected {
            "Connected to Spooderfy! We hope you enjoy your time here!"
        } else {
            "Failed to connect to Spooderfy, please try again later."
        };

        let button = if self.connected {
            let close_cb1 = self.link.callback(|_| WsEventMessages::Hide);
            let close_cb2 = self.link.callback(|_| WsEventMessages::Hide);

            let task = TimeoutService::spawn(
                Duration::from_secs(5),
                close_cb1,
            );

            self.pending_tasks.push(task);

            html! {
                <button onclick=close_cb2 class="float-right text-white border-2 rounded-lg focus:outline-none w-8 h-8">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </button>
            }
        } else {
            html!{}
        };

        html!{
            <div class="animate-slide fixed bottom-0 flex justify-center w-full ">
                <div class=div_style>
                    <h1 class="text-white font-bold w-3/4">
                        { msg }
                    </h1>
                    { button }
                </div>
            </div>
        }
    }
}


#[wasm_bindgen(start)]
pub fn run_app() {
    App::<MovieRoom>::new().mount_to_body();
}