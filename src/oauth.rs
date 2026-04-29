use crate::config::{CONSUMER_KEY, CONSUMER_SECRET, Config};
use crate::errors::{ReeknoteError, Result};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OAuthToken {
    pub token: String,
    pub secret: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OAuthClient {
    base_url: String,
    consumer_key: String,
    consumer_secret: String,
}

impl OAuthClient {
    pub fn new(config: &Config) -> Self {
        Self {
            base_url: config.user_base_url.clone(),
            consumer_key: CONSUMER_KEY.to_string(),
            consumer_secret: CONSUMER_SECRET.to_string(),
        }
    }

    pub fn with_credentials(
        base_url: impl Into<String>,
        consumer_key: impl Into<String>,
        consumer_secret: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            consumer_key: consumer_key.into(),
            consumer_secret: consumer_secret.into(),
        }
    }

    pub fn request_token(&self, callback: &str) -> Result<OAuthToken> {
        let params = self.oauth_params(None, &[("oauth_callback", callback)]);
        let response = self.get_oauth(params)?;
        let parsed = parse_oauth_response(&response)?;
        let token = required_response_value(&parsed, "oauth_token")?;
        let secret = parsed
            .get("oauth_token_secret")
            .cloned()
            .unwrap_or_default();
        Ok(OAuthToken { token, secret })
    }

    pub fn access_token(
        &self,
        request_token: &OAuthToken,
        verifier_or_url: &str,
    ) -> Result<OAuthToken> {
        let verifier = parse_verifier(verifier_or_url)?;
        let params = self.oauth_params(
            Some(request_token),
            &[
                ("oauth_token", &request_token.token),
                ("oauth_verifier", &verifier),
            ],
        );
        let response = self.get_oauth(params)?;
        let parsed = parse_oauth_response(&response)?;
        let token = required_response_value(&parsed, "oauth_token")?;
        let secret = parsed
            .get("oauth_token_secret")
            .cloned()
            .unwrap_or_default();
        Ok(OAuthToken { token, secret })
    }

    pub fn authorization_url(&self, request_token: &str) -> String {
        format!(
            "https://{}/OAuth.action?oauth_token={}",
            self.base_url,
            percent_encode(request_token)
        )
    }

    fn get_oauth(&self, params: Vec<(String, String)>) -> Result<String> {
        let url = format!("https://{}/oauth", self.base_url);
        reqwest::blocking::Client::new()
            .get(&url)
            .query(&params)
            .send()
            .and_then(|response| response.error_for_status())
            .and_then(|response| response.text())
            .map_err(|error| ReeknoteError::External(format!("OAuth request failed: {error}")))
    }

    fn oauth_params(
        &self,
        token: Option<&OAuthToken>,
        extra: &[(&str, &str)],
    ) -> Vec<(String, String)> {
        let token_secret = token.map(|token| token.secret.as_str()).unwrap_or("");
        let mut params = vec![
            ("oauth_consumer_key".to_string(), self.consumer_key.clone()),
            (
                "oauth_signature".to_string(),
                plaintext_signature(&self.consumer_secret, token_secret),
            ),
            (
                "oauth_signature_method".to_string(),
                "PLAINTEXT".to_string(),
            ),
            ("oauth_timestamp".to_string(), unix_timestamp().to_string()),
            ("oauth_nonce".to_string(), nonce()),
        ];
        for (key, value) in extra {
            params.push(((*key).to_string(), (*value).to_string()));
        }
        params
    }
}

pub fn parse_oauth_response(response: &str) -> Result<BTreeMap<String, String>> {
    let response = response
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or(response)
        .trim();
    let mut values = BTreeMap::new();
    if response.is_empty() {
        return Ok(values);
    }
    for item in response.split('&') {
        let (key, value) = item
            .split_once('=')
            .ok_or_else(|| ReeknoteError::Parse(format!("invalid OAuth response item: {item}")))?;
        values.insert(percent_decode(key)?, percent_decode(value)?);
    }
    Ok(values)
}

pub fn parse_verifier(input: &str) -> Result<String> {
    let input = input.trim();
    if input.contains("oauth_verifier=") {
        return required_response_value(&parse_oauth_response(input)?, "oauth_verifier");
    }
    if input.is_empty() {
        return Err(ReeknoteError::InvalidInput(
            "OAuth verifier is required".to_string(),
        ));
    }
    Ok(input.to_string())
}

fn required_response_value(values: &BTreeMap<String, String>, key: &str) -> Result<String> {
    values
        .get(key)
        .filter(|value| !value.is_empty())
        .cloned()
        .ok_or_else(|| ReeknoteError::External(format!("OAuth response did not contain {key}")))
}

fn plaintext_signature(consumer_secret: &str, token_secret: &str) -> String {
    format!(
        "{}&{}",
        percent_encode(consumer_secret),
        percent_encode(token_secret)
    )
}

fn percent_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                output.push(*byte as char)
            }
            byte => output.push_str(&format!("%{byte:02X}")),
        }
    }
    output
}

fn percent_decode(value: &str) -> Result<String> {
    let bytes = value.as_bytes();
    let mut output = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).map_err(|_| {
                    ReeknoteError::Parse(format!("invalid percent escape in OAuth value: {value}"))
                })?;
                let byte = u8::from_str_radix(hex, 16).map_err(|_| {
                    ReeknoteError::Parse(format!("invalid percent escape in OAuth value: {value}"))
                })?;
                output.push(byte);
                index += 3;
            }
            b'%' => {
                return Err(ReeknoteError::Parse(format!(
                    "invalid percent escape in OAuth value: {value}"
                )));
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(output)
        .map_err(|_| ReeknoteError::Parse(format!("OAuth value is not valid UTF-8: {value}")))
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn nonce() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_oauth_response_body_and_url() {
        let parsed =
            parse_oauth_response("oauth_token=tmp%20token&oauth_token_secret=secret%2Bvalue")
                .unwrap();
        assert_eq!(parsed["oauth_token"], "tmp token");
        assert_eq!(parsed["oauth_token_secret"], "secret+value");

        let parsed =
            parse_oauth_response("https://www.evernote.com/?oauth_verifier=verifier%201").unwrap();
        assert_eq!(parsed["oauth_verifier"], "verifier 1");
    }

    #[test]
    fn parses_plain_or_url_verifier() {
        assert_eq!(parse_verifier("abc").unwrap(), "abc");
        assert_eq!(
            parse_verifier("https://www.evernote.com/?oauth_token=t&oauth_verifier=v").unwrap(),
            "v"
        );
    }

    #[test]
    fn builds_authorization_url_and_plaintext_signature() {
        let client = OAuthClient::with_credentials("www.evernote.com", "key", "secret value");
        assert_eq!(
            client.authorization_url("tmp token"),
            "https://www.evernote.com/OAuth.action?oauth_token=tmp%20token"
        );
        assert_eq!(
            plaintext_signature("secret value", "tok+sec"),
            "secret%20value&tok%2Bsec"
        );
    }
}
