use apatite_api::twitch_api::AuthError;
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use std::io::BufRead as _;
use std::io::BufReader;
use std::io::Write;
use std::net::TcpListener;

use crate::config::AuthConfig;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

// ======================= PUBLIC ENTRY =======================

pub async fn authenticate(config: &AuthConfig) -> Result<Token, AuthError> {
    // Try load existing token
    if let Ok(data) = fs::read_to_string("token.json")
        && let Ok(token) = serde_json::from_str::<Token>(&data)
    {
        println!("Loaded saved token");

        // Try refresh
        if let Ok(new_token) = refresh_token(config, &token).await {
            println!("Token refreshed");
            save_token(&new_token)?;
            return Ok(new_token);
        }

        println!("Using existing token (refresh failed)");
        return Ok(token);
    }

    // Otherwise: full OAuth flow
    oauth_flow(config).await
}

// ======================= OAUTH FLOW =======================

async fn oauth_flow(config: &AuthConfig) -> Result<Token, AuthError> {
    println!("Starting OAuth flow...");

    let scopes = "user:read:email+channel:bot+user:read:chat+user:write:chat+user:bot";
    let auth_url = format!(
        "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}",
        config.client_id,
        urlencoding::encode(&config.redirect_uri),
        scopes,
    );

    println!("\nOpen this URL in your browser:\n{}\n", auth_url);

    let code = wait_for_callback();

    println!("Received authorization code");

    let client = reqwest::Client::new();

    let res: serde_json::Value = client
        .post("https://id.twitch.tv/oauth2/token")
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("code", &code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", config.redirect_uri.as_str()),
        ])
        .send()
        .await
        .map_err(|_| AuthError::RequestError)?
        .json()
        .await
        .map_err(|_| AuthError::ParseError)?;

    let token = Token {
        access_token: res["access_token"]
            .as_str()
            .ok_or(AuthError::ParseError)?
            .to_string(),
        refresh_token: res["refresh_token"]
            .as_str()
            .ok_or(AuthError::ParseError)?
            .to_string(),
        expires_in: res["expires_in"].as_u64().unwrap_or(0),
    };

    match save_token(&token) {
        Ok(_) => (),
        Err(_) => println!("Couldn't save token!"),
    };

    println!("Authentication successful ");

    Ok(token)
}

// ======================= REFRESH =======================

pub async fn refresh_token(config: &AuthConfig, token: &Token) -> Result<Token, AuthError> {
    let client = reqwest::Client::new();

    let res = client
        .post("https://id.twitch.tv/oauth2/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", token.refresh_token.as_str()),
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
        ])
        .send()
        .await
        .map_err(|_| AuthError::RequestError)?;

    if !res.status().is_success() {
        // TODO: What if rate limited? Just retry
        return Err(AuthError::RequestError);
    }

    let json: serde_json::Value = res.json().await.map_err(|_| AuthError::ParseError)?;

    Ok(Token {
        access_token: json["access_token"]
            .as_str()
            .ok_or(AuthError::ParseError)?
            .to_string(),
        refresh_token: json["refresh_token"]
            .as_str()
            .ok_or(AuthError::ParseError)?
            .to_string(),
        expires_in: json["expires_in"].as_u64().unwrap_or(0),
    })
}

// ======================= CALLBACK SERVER =======================

#[allow(clippy::expect_used, clippy::unwrap_used)]
fn wait_for_callback() -> String {
    let listener = TcpListener::bind("127.0.0.1:3000").expect("Failed to bind to localhost:3000");

    println!("Waiting for OAuth callback on http://localhost:3000...");

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();

        let buf_reader = BufReader::new(&stream);

        let request: Vec<_> = buf_reader
            .lines()
            .map(|result| result.unwrap())
            .take_while(|line| !line.is_empty())
            .collect();
        dbg!(&request);

        let status_line = request.first().unwrap();
        if let Some(start) = status_line.find("GET /callback?code=") {
            let code_start = start + "GET /callback?code=".len();
            let code_end = status_line[code_start..]
                .find('&')
                .or_else(|| status_line[code_start..].find(' '))
                .unwrap();

            let code = &status_line[code_start..code_start + code_end];

            let response = "HTTP/1.1 200 OK\r\n\r\nYou can close this tab.";
            stream.write_all(response.as_bytes()).unwrap();

            return code.to_string();
        } else if let Some(start) = status_line.find("GET /callback?error=") {
            let error_start = start + "GET /callback?error=".len();
            let error_end = status_line[error_start..]
                .find('&')
                .or_else(|| status_line[error_start..].find(' '))
                .unwrap();
            let error = &status_line[error_start..error_start + error_end];

            let response = "HTTP/1.1 200 OK\r\n\r\nError: .".to_owned() + error;
            stream.write_all(response.as_bytes()).unwrap();
        }
    }

    panic!("Failed to receive OAuth callback");
}

// ======================= UTIL =======================

fn save_token(token: &Token) -> Result<(), AuthError> {
    let data = serde_json::to_string_pretty(token).map_err(|_| AuthError::SaveError)?;
    fs::write("token.json", data).map_err(|_| AuthError::SaveError)?;
    Ok(())
}
