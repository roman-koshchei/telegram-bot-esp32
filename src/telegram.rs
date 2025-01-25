use alloc::format;
use embedded_io_async::{Read, Write};
use log::info;
use reqwless::{client::HttpResource, headers::ContentType, request::RequestBuilder as _};
use serde_json_core as _;

pub struct Client<'a, C>
where
    C: Read + Write,
{
    http: reqwless::client::HttpResource<'a, C>,
    bot_token: &'a str,
    chat_id: &'a str,
    response_buffer: [u8; 8196],
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
            response_buffer: [0; 8196],
        }
    }

    pub async fn send_message(&mut self, text: &str, is_html: bool) -> bool {
        let parse_mode = if is_html { "HTML" } else { "MarkdownV2" };
        let path = format!("/bot{}/sendMessage", self.bot_token);

        let mut body_buffer = [0; 2048];
        let size = serde_json_core::ser::to_slice(
            &SendMessageBody {
                chat_id: self.chat_id,
                text,
                parse_mode,
                protect_content: true,
            },
            &mut body_buffer,
        )
        .expect("Body Buffer is too small"); // TODO: rework without panic
        // info!("TG: send_message request body size: {}", size);

        let res = self
            .http
            .post(&path)
            .body(&body_buffer[..])
            .content_type(ContentType::ApplicationJson)
            .send(&mut self.response_buffer)
            .await;
        // log::info!("TG: send_message response");

        res.is_ok_and(|x| x.status.is_successful())
    }

    pub async fn get_updates(&mut self, offset: i64) -> Option<TelegramUpdates> {
        let path = format!("/bot{}/getUpdates?offset={}", self.bot_token, offset);

        let response = self
            .http
            .get(&path)
            .send(&mut self.response_buffer)
            .await
            .ok()?;
        // log::info!("TG: get_updates got response");

        let body = response.body().read_to_end().await.ok()?;
        // log::info!("TG: get_updates response body size: {}", body.len());

        let serialized = serde_json_core::from_slice::<TelegramUpdates>(body)
            .expect("TG: get_updates serialize failed");
        Some(serialized.0)
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
    // pub text: heapless::String<256>,
    // f*cking serde can't cut string to 256 size.
    // it just errors. stupid sh*t
    pub text: alloc::string::String,
}

#[derive(serde::Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

#[derive(serde::Deserialize)]
pub struct TelegramUpdates {
    pub result: alloc::vec::Vec<TelegramUpdate>,
}
