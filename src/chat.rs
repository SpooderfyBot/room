use yew::prelude::*;
use yew::services::ConsoleService;

use reqwest::Client;
use serde::{Serialize, Deserialize};

use crate::opcodes;
use crate::settings;
use crate::utils::{send_future, start_future, emit_event};
use crate::websocket::{WsHandler, WebsocketMessage, WrappingWsMessage};



#[derive(Properties, Clone)]
pub struct ChatRoomProperties {
    /// The room websocket handle.
    pub ws: WsHandler,

    /// The room id.
    pub room_id: String,
}


/// The chat display for messages.
///
/// The room subscribes to the MESSAGE event from the websocket and
/// appends the message to the list on a event, this list is never
/// cleared.
pub struct ChatRoom {
   _ws: WsHandler,
    room_id: String,
    messages: Vec<Message>,
}

impl ChatRoom {
    /// A simple callback that is invoked when a message is received via the
    /// websocket, the view is always re-rendered after this operation.
    pub fn on_message(&mut self, message: Message) {
        self.messages.push(message);
    }
}

impl Component for ChatRoom {
    type Message = WebsocketMessage;
    type Properties = ChatRoomProperties;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {

        let messages = vec![];

        let ws = props.ws;
        let ws_cb = link.callback(|msg| msg);

        ws.subscribe_to_message(
            settings::CHAT_ID,
            opcodes::OP_MESSAGE,
            ws_cb
        );

        Self {
            _ws: ws,
            room_id: props.room_id,
            messages,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        let content = match msg {
            WebsocketMessage::Empty => return false,
            WebsocketMessage::Payload(value) => value,
        };

        let msg: Message = serde_json::from_value(content)
            .unwrap();

        self.on_message(msg);

        true
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        html! {
            <div class="min-h-full w-1/3 p-4">
                <div class="flex flex-col bg-discord-dark rounded-lg h-full p-4">
                    <div class="h-full pt-1">
                        { for self.messages.iter().map(|msg| {msg.to_html()}) }
                    </div>
                    <div class="self-end h-auto w-full">
                        <TextInput room_id=self.room_id.clone()/>
                    </div>
                </div>
            </div>
        }
    }
}


/// Represents a standard chat message, the client is aware of what it is
/// and sends itself to other clients with the containing info in order
/// to produce the P2P behaviour.
#[derive(Serialize, Deserialize)]
pub struct Message {
    /// The Discord user's display name e.g. Cf8
    username: String,

    /// The user's full avatar url.
    avatar: String,

    /// The content of the message.
    content: String,
}

impl Message {
    /// Renders the message to a html element.
    fn to_html(&self) -> Html {
        html! {
            <div class="flex py-2">
                <img class="inline-block rounded-full h-12 w-12" src={&self.avatar} alt="" />
                <div class="inline-block px-3 w-5/6">
                    <h1 class="text-blue-400 font-semibold">{ &self.username }</h1>
                    <p class="text-white" style="word-wrap: break-word;">
                        { &self.content }
                    </p>
                </div>
            </div>
        }
    }
}


/// Fetches the user data with a given session, this allows the text input
/// to know who they are as a user.
async fn who_am_i() -> TextInputEvents {
    let url = settings::get_who_am_i_url();

    let resp = Client::new()
        .get(&url)
        .send()
        .await;

    if let Ok(resp) = resp {
        let user = resp.json::<UserInfo>().await.unwrap();
        TextInputEvents::WhoAmI(user)
    } else {
        TextInputEvents::RequestError
    }
}


/// Fetches the webhook info for the message system to allow messages to
/// discord.
async fn acquire_webhook(room_id: String) -> TextInputEvents {
    let url = settings::get_webhook_api(&room_id);

    let resp = Client::new()
        .get(&url)
        .send()
        .await;

    if let Ok(resp) = resp {
        let wh = resp.json::<Webhook>().await.unwrap();
        TextInputEvents::Webhook(wh)
    } else {
        TextInputEvents::RequestError
    }
}

/// Sends a PUT request to the api to emit a message to clients.
async fn send_message(room_id: String, wh_url: String, msg: Message) {
    {
        let webhook_payload = WebhookMessage {
            username: &msg.username,
            avatar_url: &msg.avatar,
            content: &msg.content,
        };

        let _ = Client::new()
            .post(&wh_url)
            .json(&webhook_payload)
            .send()
            .await;
    }


    let msg = serde_json::to_value(msg).unwrap();
    let payload = WrappingWsMessage {
        opcode: opcodes::OP_MESSAGE,
        payload: Some(msg)
    };

    emit_event(room_id, payload).await;
}


#[derive(Serialize)]
struct WebhookMessage<'a>{
    username: &'a str,
    avatar_url: &'a str,
    content: &'a str,
}


/// The info of a the active user.
///
/// This is fetched via the @me endpoint and is used to emit events
/// later on from the text input component.
#[derive(Debug, Deserialize)]
pub struct UserInfo {
    username: String,
    avatar: String,
}


/// The room webhook for Discord.
#[derive(Debug, Deserialize)]
pub struct Webhook {
    url: String,
}


#[derive(Properties, Clone)]
pub struct TextInputProperties {
    pub room_id: String,
}

/// Text input events either from a button click or text input.
#[derive(Debug)]
pub enum TextInputEvents {
    /// A text input key press.
    KeyPress(String),

    /// The submit button has been pressed.
    Submit,

    /// The user identification result.
    WhoAmI(UserInfo),

    /// The user identification result.
    Webhook(Webhook),

    /// The request lookup failed.
    RequestError,
}

pub struct TextInput {
    link: ComponentLink<Self>,
    room_id: String,
    msg: Vec<String>,
    user: Option<UserInfo>,
    webhook_url: String,
}

impl Component for TextInput {
    type Message = TextInputEvents;
    type Properties = TextInputProperties;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        // get who we are.
        send_future(
            link.clone(),
            who_am_i()
        );
        send_future(
            link.clone(),
            acquire_webhook(props.room_id.clone())
        );

        Self {
            link,
            room_id: props.room_id,
            msg: Vec::with_capacity(1024),
            user: None,
            webhook_url: "".to_string(),
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            TextInputEvents::Submit => return self.submit(),
            TextInputEvents::KeyPress(key) => {
                if let None = self.user {
                    return true;
                }

                if &key == "Enter" {
                    return self.submit();
                }

                if self.msg.len() < 1024 {
                    self.msg.push(key);
                }
            },
            TextInputEvents::WhoAmI(user) => {
                self.user = Some(user);
            },
            TextInputEvents::Webhook(wh) => {
                self.webhook_url = wh.url;
            }
            TextInputEvents::RequestError => {
                ConsoleService::error("Failed to get request");
            },
        }

        false
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        let typing_cb = self.link.callback(
            |e: KeyboardEvent| TextInputEvents::KeyPress(e.key())
        );
        let submit_cb = self.link.callback(
            |_| TextInputEvents::Submit
        );

        let existing: String = self.msg.join("");

        html! {
            <div class="p-2 relative w-full">
                <label>
                    <input
                        class="\
                            transition duration-300 linear \
                            border-2 border-blue-800 focus:border-blue-600 \
                            text-white text-sm font-medium placeholder-gray-200 \
                            rounded-lg focus:outline-none \
                            bg-gray-800 w-full h-10 px-5 pr-16"
                        onkeypress=typing_cb
                        value=existing
                        name="message"
                        placeholder="Send something to the movie room..."
                        type="text"
                    />
               </label>
               <button onclick=submit_cb class="absolute right-0 top-0 my-4 mr-4 focus:outline-none"
                       type="submit">
               </button>
            </div>
        }
    }
}

impl TextInput {
    /// Joins the characters of the message together, clears the vector
    /// and sends the message to the gateway if the `user` field is not
    /// None, in the case that it is None; nothing happens.
    fn submit(&mut self) -> ShouldRender {
        if let Some(user) = self.user.as_ref() {
            let complete_msg: String = self.msg.join("");
            self.msg.clear();

            let msg = Message {
                username: user.username.clone(),
                avatar: user.avatar.clone(),
                content: complete_msg,
            };

            start_future(send_message(
                self.room_id.clone(),
                self.webhook_url.clone(), msg));


            true
        } else {
            false
        }
    }
}



