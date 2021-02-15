use yew::prelude::*;
use yew::services::{ConsoleService, TimeoutService};
use yew::services::timeout::TimeoutTask;

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;

use serde::Deserialize;
use reqwest::Client;
use wasm_bindgen::closure::Closure;

use crate::opcodes;
use crate::settings;
use crate::websocket::{WsHandler, WebsocketMessage};
use crate::binder;
use crate::utils;


/// The set component properties that can be set by the parent component.
#[derive(Properties, Clone)]
pub struct MediaPlayerProperties {
    /// The WS handle for subscribing to events.
    pub ws: WsHandler,

    /// The room id of the given room.
    pub room_id: String,
}


pub enum MediaPlayerEvent {
    LiveStatus(bool),
    StatsUpdate(WebsocketMessage),
    ReCheckVideo,
    GotStreamUrl(String),
}

#[derive(Deserialize)]
struct StreamUrlResp {
    stream_url: String,
}

#[derive(Deserialize)]
struct Stats {
    members: usize,
    multiplier: String,
}


#[derive(Deserialize)]
struct VideoInfo {
    owner: String,
    title: String,
}


/// The video player and details component.
///
/// This displays the help page of the player if no videos are added or set
/// otherwise it shows the video of the currently selected track according
/// to what all the other players are set to.
///
/// This components uses the VideoPlayer component to extend its base and
/// handle the actual video events itself, this just displays the title
/// and gives controls for track selection.
pub struct MediaPlayer {
    /// The player component link for callbacks
    link: ComponentLink<Self>,

    /// If the ws is connected or not
    is_connected: bool,

    /// The stats of the room.
    stats: Stats,

    /// Info about the room.
    info: VideoInfo,

    task: Option<TimeoutTask>,

    events_set: AtomicBool,

    callback: (Closure<dyn FnMut()>, Closure<dyn FnMut()>),

    timer: usize,

    stream_url: String,
}

impl Component for MediaPlayer {
    type Message = MediaPlayerEvent;
    type Properties = MediaPlayerProperties;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let event_cb = link.callback(
            |event| MediaPlayerEvent::StatsUpdate(event)
        );

        let ws = props.ws;
        ws.subscribe_to_message(settings::PLAYER_ID, opcodes::OP_STATS_UPDATE, event_cb);

        let stream_url_cb = link.callback(|url| MediaPlayerEvent::GotStreamUrl(url));
        let url = settings::get_stream_api_url(&props.room_id);
        utils::start_with_cb(stream_url_cb, async move {
            let get_url = url;
            let resp = Client::new()
                .get(&get_url)
                .send()
                .await;

            return if let Ok(resp) = resp {
                if let Ok(info) = resp.json::<StreamUrlResp>().await {
                    info.stream_url
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
            }
        });

        let error_cb = link.callback(|_| MediaPlayerEvent::LiveStatus(false));
        let connected_cb = link.callback(|_| MediaPlayerEvent::LiveStatus(true));

        let on_error = Closure::wrap(Box::from(move || error_cb.emit(())) as Box<dyn FnMut()>);
        let on_meta = Closure::wrap(Box::from(move || connected_cb.emit(())) as Box<dyn FnMut()>);
        let is_set = binder::set_listeners(&on_error, &on_meta);

        let stats = Stats {
            members: 1,
            multiplier: "1x".to_string(),
        };
        
        let info = VideoInfo {
            owner: "ハーリさん (CF8)".to_string(),
            title: "Some Stream".to_string()
        };

        Self {
            link: link.clone(),
            is_connected: false,
            stats,
            info,
            task: None,
            events_set: AtomicBool::new(is_set),
            callback: (on_error, on_meta),
            timer: 0,
            stream_url: "".to_string()
        }
    }

    /// Handles the media player events based off the Websocket and localised
    /// events.
    ///
    /// `MediaPlayerEvent::Next` and `MediaPlayerEvent::Previous` both contain
    /// a bool to signal if they should emit events to the gateway or not
    /// this is because both the user callbacks and websocket callbacks are
    /// the same just with a different bool signal, this is to cut down the
    /// size of the code base and keep it simple as unlike the video player
    /// these are not massively specialised.
    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            MediaPlayerEvent::StatsUpdate(val) => {
                if let Some(stats) = val.unwrap_and_into::<Stats>() {
                    self.stats = stats
                } else {
                    ConsoleService::warn("Failed to parse status update in player");
                };
            },
            MediaPlayerEvent::LiveStatus(is_live) => {
                if !is_live {
                    if self.timer < ((10 * 6) * 2) {
                        self.task = Some(TimeoutService::spawn(
                            Duration::from_secs(10),
                            self.link.callback(|_| MediaPlayerEvent::ReCheckVideo)
                        ));
                    }
                }
                self.is_connected = is_live;
                ConsoleService::log("status change");
            },
            MediaPlayerEvent::ReCheckVideo => {
                self.timer += 10;
                binder::try_reload();
                ConsoleService::log("reloading");
            },
            MediaPlayerEvent::GotStreamUrl(url) => {
                self.stream_url = url;
            }
        }

        true
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }

    /// Renders the whole media player half of the page.
    ///
    /// This displays the help page of the player if no videos are added or set
    /// otherwise it shows the video of the currently selected track according
    /// to what all the other players are set to.
    ///
    /// This components uses the VideoPlayer component to extend its base and
    /// handle the actual video events itself, this just displays the title
    /// and gives controls for track selection.
    fn view(&self) -> Html {
        if !self.events_set.load(Relaxed) {
            let is_set = binder::set_listeners(&self.callback.0, &self.callback.1);
            self.events_set.store(is_set, Relaxed);
        }


        let status = if self.is_connected {
            html!{
                <div class="text-white text-lg font-semibold flex items-center">
                    <div class="inline-block bg-green-500 border-2 border-green-400 rounded-full w-2 h-2 p-1 mt-1 mx-2"></div>
                    {"online"}
                </div>
            }
        } else {
            html!{
                <div class="text-white text-lg font-semibold flex items-center">
                    <div class="inline-block bg-red-500 border-2 border-red-400 rounded-full w-2 h-2 p-1 mt-1 mx-2"></div>
                    {"offline"}
                </div>
            }
        };


        let members = html! {
            <div class="flex justify-center items-center mx-2">
                <div class="w-5 h-5 object-contain text-white mx-2">
                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
                      <path d="M13 6a3 3 0 11-6 0 3 3 0 016 0zM18 8a2 2 0 11-4 0 2 2 0 014 0zM14 15a4 4 0 00-8 0v3h8v-3zM6 8a2 2 0 11-4 0 2 2 0 014 0zM16 18v-3a5.972 5.972 0 00-.75-2.906A3.005 3.005 0 0119 15v3h-3zM4.75 12.094A5.973 5.973 0 004 15v3H1v-3a3 3 0 013.75-2.906z" />
                    </svg>
                </div>
                <h1 class="text-lg text-white font-semibold">{self.stats.members}</h1>
            </div>
        };

        let multiplier = html!{
            <div class="flex justify-center items-center mx-2">
                <div class="w-5 h-5 object-contain text-red-600 mx-2">
                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
                      <path fill-rule="evenodd" d="M12.395 2.553a1 1 0 00-1.45-.385c-.345.23-.614.558-.822.88-.214.33-.403.713-.57 1.116-.334.804-.614 1.768-.84 2.734a31.365 31.365 0 00-.613 3.58 2.64 2.64 0 01-.945-1.067c-.328-.68-.398-1.534-.398-2.654A1 1 0 005.05 6.05 6.981 6.981 0 003 11a7 7 0 1011.95-4.95c-.592-.591-.98-.985-1.348-1.467-.363-.476-.724-1.063-1.207-2.03zM12.12 15.12A3 3 0 017 13s.879.5 2.5.5c0-1 .5-4 1.25-4.5.5 1 .786 1.293 1.371 1.879A2.99 2.99 0 0113 13a2.99 2.99 0 01-.879 2.121z" clip-rule="evenodd" />
                    </svg>
                </div>
                <h1 class="text-lg text-white font-semibold">{&self.stats.multiplier}</h1>
            </div>
        };
        
        let owner_and_title = html!{
            <div class="flex justify-center items-center mx-1">
                <h1 class="text-lg text-white font-semibold">
                    {&self.info.owner} {" - "} {&self.info.title}
                </h1>
            </div>
        };

        let stats_block = html!{
            <div class="flex justify-between mb-2 px-8">
                { status }
                { owner_and_title }
                <div class="flex justify-center">
                    { members }
                    { multiplier }
                </div>
            </div>
        };

        let options = format!("{{type: 'flv', url: '{}'}}", &self.stream_url);
        let js = format!(r#"
            if (flvjs.isSupported()) {{
                var videoElement = document.getElementById('player');
                var flvPlayer = flvjs.createPlayer({});
                flvPlayer.attachMediaElement(videoElement);
                flvPlayer.load();

                videoElement.onplay = function () {{
                    setTimeout(() => {{
                        let max_pos = videoElement.seekable.end(0);
                        if ((max_pos === Infinity) || (max_pos < 1)) {{
                            max_pos = 1;
                        }}
                        videoElement.currentTime = (max_pos - 1);
                    }}, 200);
                }};
            }}
        "#, &options);

        let player_style = if self.is_connected {
            "bg-gray-900"
        } else {
            "bg-gray-900 hidden"
        };

        let poster_style = if !self.is_connected {
            "flex justify-center items-center w-full h-full bg-gray-900 rounded-lg shadow-inner"
        } else {
            "hidden"
        };

        let message = if self.timer >= ((10 * 6) * 2) {
            "The stream has gone to sleep, reload the page when the you're room owner \
            is ready to start streaming."
        } else {
            "Waiting for stream to start"
        };

        html!{
             <div class="w-2/3 h-full my-auto py-4 px-20">
                <div class="h-full bg-discord-dark rounded-lg p-4">
                    <div class="w-full mb-4">
                        { stats_block }
                        <div class="w-full border-b-4 border-white rounded-full"></div>
                    </div>
                    <div class="flex justify-center">
                        <script src="https://unpkg.com/flv.js/dist/flv.min.js"></script>
                        <video id="player" class=player_style controls=true muted=false></video>
                        <div class=poster_style style="min-height: 30vw;">
                            <div>
                                <h1 class="text-white font-bold text-4xl text-center">
                                    { message }
                                </h1>
                                <div class="flex justify-center">
                                    <img class="w-64 h-64 object-contain rounded-full" src="https://cdn.discordapp.com/attachments/667270372042866699/805836261008211988/Spooderfy_Transparent.png" alt=""/>
                                </div>
                            </div>
                        </div>
                        <script>
                            { js }
                        </script>
                    </div>
                </div>
             </div>

        }
    }
}
