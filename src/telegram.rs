use alloc::{format, string::String};
use heapless::Vec;

pub struct Client<'a> {
    bot_token: &'a str,
    chat_id: &'a str,
}

pub struct PostRequest {
    pub path: String,
    pub body: String,
}

pub struct GetRequest {
    pub path: String,
}

pub const BASE_URL: &str = "https://api.telegram.org";

impl<'a> Client<'a> {
    pub fn new(bot_token: &'a str, chat_id: &'a str) -> Client<'a> {
        Client { bot_token, chat_id }
    }

    pub fn send_message(self: &Self, text: &str, is_html: bool) -> PostRequest {
        let prase_mode = if is_html { "HTML" } else { "MarkdownV2" };
        PostRequest {
            path: format!("/bot{}/sendMessage", self.bot_token),
            body: format!(
                "{{ \"chat_id\": \"{}\", \"text\": \"{}\", \"protect_content\": true, \"parse_mode\": \"{}\" }}",
                self.chat_id, text, prase_mode
            )
        }
    }

    pub fn get_updates(self: &Self, offset: i64) -> GetRequest {
        GetRequest {
            path: format!("/bot{}/getUpdates?offset={}", self.bot_token, offset),
        }
    }
}

#[derive(serde::Deserialize)]
pub struct TelegramMessage {
    pub text: heapless::String<256>,
}

#[derive(serde::Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

#[derive(serde::Deserialize)]
pub struct TelegramUpdates {
    pub result: Vec<TelegramUpdate, 10>,
}
