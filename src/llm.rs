use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::time::Duration;

pub struct Llm {
    agent: ureq::Agent,
    url: String,
    key: Option<String>,
    temperature: Option<f64>,
    max_tokens: usize,
    retries: usize,
    json_mode: bool,
    site_url: Option<String>,
    app_name: String,
}

pub struct Options {
    pub base_url: String,
    pub key: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: usize,
    pub retries: usize,
    pub timeout_seconds: u64,
    pub json_mode: bool,
    pub site_url: Option<String>,
    pub app_name: String,
}

impl Llm {
    pub fn new(options: Options) -> Self {
        let base = options.base_url.trim_end_matches('/');
        let url = if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{base}/chat/completions")
        };
        let timeout = Duration::from_secs(options.timeout_seconds);
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(timeout)
            .timeout_read(timeout)
            .timeout_write(timeout)
            .build();
        Self {
            agent,
            url,
            key: options.key,
            temperature: options.temperature,
            max_tokens: options.max_tokens,
            retries: options.retries,
            json_mode: options.json_mode,
            site_url: options.site_url,
            app_name: options.app_name,
        }
    }

    fn chat(&self, model: &str, messages: &[Value]) -> Result<String, String> {
        let mut body = json!({
            "model": model,
            "messages": messages,
            "max_tokens": self.max_tokens,
            "stream": false
        });
        if let Some(temperature) = self.temperature {
            body["temperature"] = json!(temperature);
        }
        if self.json_mode {
            body["response_format"] = json!({ "type": "json_object" });
        }

        let mut request = self
            .agent
            .post(&self.url)
            .set("Content-Type", "application/json")
            .set("User-Agent", "SLOP/1.0")
            .set("X-Title", &self.app_name);
        if let Some(key) = &self.key {
            request = request.set("Authorization", &format!("Bearer {key}"));
        }
        if let Some(site_url) = &self.site_url {
            request = request.set("HTTP-Referer", site_url);
        }

        let response: Value = match request.send_json(body) {
            Ok(response) => response
                .into_json()
                .map_err(|error| format!("{model}: invalid response JSON: {error}"))?,
            Err(ureq::Error::Status(code, response)) => {
                let detail = response.into_string().unwrap_or_default();
                return Err(format!("{model}: HTTP {code}: {detail}"));
            }
            Err(error) => return Err(format!("{model}: {error}")),
        };
        message_text(&response).ok_or_else(|| {
            let detail = response["choices"][0]["message"]["refusal"]
                .as_str()
                .unwrap_or("no text content");
            format!("{model}: {detail} in API response")
        })
    }

    pub fn ask<T: DeserializeOwned, U>(
        &self,
        model: &str,
        role: &str,
        user: String,
        check: impl Fn(T) -> Result<U, String>,
    ) -> Result<U, String> {
        let mut messages = vec![
            json!({ "role": "system", "content": crate::prompts::system(role) }),
            json!({ "role": "user", "content": user }),
        ];
        let mut last_problem = String::new();
        for _ in 0..=self.retries {
            let reply = self.chat(model, &messages)?;
            match parse_json::<T>(&reply).and_then(&check) {
                Ok(value) => return Ok(value),
                Err(problem) => {
                    messages.push(json!({ "role": "assistant", "content": reply }));
                    messages.push(json!({
                        "role": "user",
                        "content": format!(
                            "The response envelope could not be read: {problem}. Return only one corrected JSON object."
                        )
                    }));
                    last_problem = problem;
                }
            }
        }
        Err(format!(
            "{model} returned an unreadable response {} times; last error: {last_problem}",
            self.retries + 1
        ))
    }
}

fn message_text(response: &Value) -> Option<String> {
    let content = &response["choices"][0]["message"]["content"];
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }
    let parts = content.as_array()?;
    let text = parts
        .iter()
        .filter_map(|part| part["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    (!text.is_empty()).then_some(text)
}

fn parse_json<T: DeserializeOwned>(reply: &str) -> Result<T, String> {
    let reply = reply.rsplit("</think>").next().unwrap_or(reply).trim();
    let start = reply
        .find('{')
        .ok_or_else(|| "no JSON object".to_string())?;
    let end = reply
        .rfind('}')
        .filter(|end| *end > start)
        .ok_or_else(|| "unterminated JSON object".to_string())?;
    serde_json::from_str(&reply[start..=end]).map_err(|error| format!("invalid JSON ({error})"))
}

#[cfg(test)]
mod tests {
    use super::{message_text, parse_json};
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Deserialize)]
    struct Answer {
        value: bool,
    }

    #[test]
    fn extracts_json_after_reasoning() {
        let answer: Answer = parse_json("<think>hmm</think>\n{\"value\":true}").unwrap();
        assert!(answer.value);
    }

    #[test]
    fn reads_string_and_block_content() {
        let string = json!({"choices":[{"message":{"content":"hello"}}]});
        let blocks = json!({"choices":[{"message":{"content":[{"type":"text","text":"hel"},{"type":"text","text":"lo"}]}}]});
        assert_eq!(message_text(&string).unwrap(), "hello");
        assert_eq!(message_text(&blocks).unwrap(), "hello");
    }
}
