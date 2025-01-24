use alloc::{format, string::String};

pub struct Client<'a> {
    bot_token: &'a str,
    chat_id: &'a str,
}

pub struct PostRequest {
    pub url: String,
    pub body: String,
}

pub struct GetRequest {
    pub url: String,
}

impl<'a> Client<'a> {
    pub fn new(bot_token: &'a str, chat_id: &'a str) -> Client<'a> {
        Client { bot_token, chat_id }
    }

    pub fn send_message(self: &Self, text: &str, is_html: bool) -> PostRequest {
        let prase_mode = if is_html { "HTML" } else { "MarkdownV2" };
        PostRequest {
            url: format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token),
            body: format!(
                "{{ \"chat_id\": \"{}\", \"text\": \"{}\", \"protect_content\": true, \"parse_mode\": \"{}\" }}",
                self.chat_id, text, prase_mode
            )
        }
    }

    pub fn get_updates(self: &Self, offset: i64) -> GetRequest {
        GetRequest {
            url: format!(
                "https://api.telegram.org/bot{}/getUpdates?offset={}",
                self.bot_token, offset
            ),
        }
    }
}
