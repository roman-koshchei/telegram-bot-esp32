use alloc::{format, string::ToString};
use embedded_nal_async::{Dns, TcpConnect};
// use log;
use reqwless::{
    client::{HttpClient, HttpResource},
    headers::ContentType,
    request::RequestBuilder as _,
    response::StatusCode,
};
use serde_json_core as _;

pub struct Client<'a, T, D>
where
    T: TcpConnect + 'a,
    D: Dns + 'a,
{
    client: HttpClient<'a, T, D>,
    bot_token: &'a str,
    chat_id: &'a str,
    response_buffer: [u8; 8196],
}

pub const HOSTNAME: &str = "https://api.telegram.org";

impl<'a, T, D> Client<'a, T, D>
where
    T: TcpConnect + 'a,
    D: Dns + 'a,
{
    pub fn new(
        // for http
        tcp: &'a T,
        dns: &'a D,
        tls: reqwless::client::TlsConfig<'a>,

        // secrets
        bot_token: &'a str,
        chat_id: &'a str,
    ) -> Self {
        Self {
            client: HttpClient::new_with_tls(tcp, dns, tls),
            bot_token,
            chat_id,
            response_buffer: [0; 8196],
        }
    }

    // only with Polonius
    // async fn resource(
    //     client: &'a mut HttpClient<'a, T, D>,
    // ) -> Option<HttpResource<'a, T::Connection<'a>>> {
    //     const MAX_ATTEMPTS: u8 = 8;
    //     let mut attempt: u8 = 0;
    //     loop {
    //         attempt += 1;
    //         match client.resource(HOSTNAME).await {
    //             Ok(res) => {
    //                 return Some(res);
    //             }
    //             Err(_) => {
    //                 if attempt >= MAX_ATTEMPTS {
    //                     return None;
    //                 }
    //             }
    //         }
    //     }
    // }

    pub async fn send_message(
        &mut self,
        text: &str,
        is_html: bool,
    ) -> Result<(), SendMessageError> {
        let parse_mode = if is_html { "HTML" } else { "MarkdownV2" };
        let path = format!("/bot{}/sendMessage", self.bot_token);

        let mut body_buffer = [0; 2048];
        let _ = serde_json_core::ser::to_slice(
            &SendMessageBody {
                chat_id: self.chat_id,
                text,
                parse_mode,
                protect_content: true,
            },
            &mut body_buffer,
        )
        .map_err(|_| SendMessageError::TooSmallBodyBuffer)?;
        // info!("TG: send_message request body size: {}", size);

        let mut resource = self
            .client
            .resource(HOSTNAME)
            .await
            .map_err(SendMessageError::ReqwlessError)?;

        let response = resource
            .post(&path)
            .body(&body_buffer[..])
            .content_type(ContentType::ApplicationJson)
            .send(&mut self.response_buffer)
            .await
            .map_err(SendMessageError::ReqwlessError)?;
        // log::info!("TG: send_message response");

        if response.status.is_successful() {
            Ok(())
        } else {
            let status = response.status;
            let body = response.body().read_to_end().await.unwrap();
            let str_body = alloc::str::from_utf8(&body).unwrap();
            log::error!("{}", str_body);

            Err(SendMessageError::StatusCodeIsNotSuccessful(status))
        }
    }

    pub async fn get_updates(&mut self, offset: i64) -> Result<TelegramUpdates, GetUpdatesError> {
        let path = format!("/bot{}/getUpdates?offset={}", self.bot_token, offset);

        let mut resource = self
            .client
            .resource(HOSTNAME)
            .await
            .map_err(GetUpdatesError::ReqwlessError)?;

        let response = resource
            .get(&path)
            .send(&mut self.response_buffer)
            .await
            .map_err(GetUpdatesError::ReqwlessError)?;
        // log::info!("TG: get_updates got response");

        let body = response
            .body()
            .read_to_end()
            .await
            .map_err(GetUpdatesError::ReqwlessError)?;
        // log::info!("TG: get_updates response body size: {}", body.len());

        let serialized = serde_json_core::from_slice::<TelegramUpdates>(body)
            .map_err(|_| GetUpdatesError::DeserializationFailed)?;

        Ok(serialized.0)
    }
}

#[derive(serde::Serialize)]
struct SendMessageBody<'a> {
    chat_id: &'a str,
    text: &'a str,
    protect_content: bool,
    parse_mode: &'a str,
}

pub enum SendMessageError {
    TooSmallBodyBuffer,
    StatusCodeIsNotSuccessful(StatusCode),
    ReqwlessError(reqwless::Error),
}

pub enum GetUpdatesError {
    DeserializationFailed,
    ReqwlessError(reqwless::Error),
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
