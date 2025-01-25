use alloc::{format, string::String};
use embedded_io_async::{Read, Write};
use heapless::Vec;
use reqwless::{client::HttpResource, headers::ContentType, request::RequestBuilder as _};
use serde::de::value;

pub struct Client<'a, C>
where
    C: Read + Write,
{
    http: reqwless::client::HttpResource<'a, C>,
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

impl<'a, C> Client<'a, C>
where
    C: Read + Write,
{
    pub fn new(http: HttpResource<'a, C>, bot_token: &'a str, chat_id: &'a str) -> Self {
        Self {
            http,
            bot_token,
            chat_id,
        }
    }

    pub async fn send_message(&mut self, text: &str, is_html: bool) -> bool {
        let parse_mode = if is_html { "HTML" } else { "MarkdownV2" };
        let path = format!("/bot{}/sendMessage", self.bot_token);

        let mut body_buffer = [0u8; 4096];
        let _ = serde_json_core::ser::to_slice(
            &SendMessageBody {
                chat_id: self.chat_id,
                text,
                parse_mode,
                protect_content: true,
            },
            &mut body_buffer,
        )
        .expect("Body Buffer is too small"); // TODO: rework without panic

        let mut res_buffer = [0u8; 4096];
        let res = self
            .http
            .post(&path)
            .body(&body_buffer[..])
            .content_type(ContentType::ApplicationJson)
            .send(&mut res_buffer)
            .await;

        return res.is_ok_and(|x| x.status.is_successful());
    }

    pub async fn get_updates(&mut self, offset: i64) -> Option<TelegramUpdates> {
        let path = format!("/bot{}/getUpdates?offset={}", self.bot_token, offset);

        let mut buffer = [0u8; 4096];
        let response = self.http.get(&path).send(&mut buffer).await.ok()?;
        let body = response.body().read_to_end().await.ok()?;
        serde_json_core::from_slice::<TelegramUpdates>(body)
            .map(|x| x.0)
            .ok()
    }
}

#[derive(serde::Serialize)]
struct SendMessageBody<'a> {
    chat_id: &'a str,
    text: &'a str,
    protect_content: bool,
    parse_mode: &'a str,
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
