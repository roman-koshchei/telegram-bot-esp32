use alloc::{format, string::String};

pub fn send_message_url(bot_token: &'static str) -> String {
    format!("https://api.telegram.org/bot{}/sendMessage", bot_token)
}

pub fn send_message_body(chat_id: &str, text: &str, is_html: bool) -> String {
    let prase_mode = if is_html { "HTML" } else { "MarkdownV2" };
    format!(
        "{{ \"chat_id\": \"{}\", \"text\": \"{}\", \"protect_content\": true, \"parse_mode\": \"{}\" }}",
        chat_id, text, prase_mode
    )
}
