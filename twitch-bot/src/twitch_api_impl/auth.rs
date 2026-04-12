use serde::Deserialize;
use serde::Serialize;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;

use crate::config::AuthConfig;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
}

// ======================= PUBLIC ENTRY =======================

pub async fn authenticate(config: &AuthConfig) -> Token {
    // Try load existing token
    if let Ok(data) = fs::read_to_string("token.json") {
        if let Ok(token) = serde_json::from_str::<Token>(&data) {
            println!("Loaded saved token");

            // Try refresh
            if let Ok(new_token) = refresh_token(config, &token).await {
                println!("Token refreshed");
                save_token(&new_token);
                return new_token;
            }

            println!("Using existing token (refresh failed)");
            return token;
        }
    }

    // Otherwise: full OAuth flow
    oauth_flow(config).await
}

// ======================= OAUTH FLOW =======================

async fn oauth_flow(config: &AuthConfig) -> Token {
    println!("Starting OAuth flow...");

    let scopes = "user:read:email+channel:bot+user:read:chat+user:write:chat+user:bot";
    let auth_url = format!(
        "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}",
        config.client_id,
        urlencoding::encode(&config.redirect_uri),
        scopes,
    );

    println!("\n👉 Open this URL in your browser:\n{}\n", auth_url);

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
        .unwrap()
        .json()
        .await
        .unwrap();

    let token = Token {
        access_token: res["access_token"].as_str().unwrap().to_string(),
        refresh_token: res["refresh_token"].as_str().unwrap().to_string(),
        expires_in: res["expires_in"].as_u64().unwrap_or(0),
    };

    save_token(&token);

    println!("Authentication successful ");

    token
}

// ======================= REFRESH =======================

pub async fn refresh_token(config: &AuthConfig, token: &Token) -> Result<Token, ()> {
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
        .map_err(|_| ())?;

    if !res.status().is_success() {
        return Err(());
    }

    let json: serde_json::Value = res.json().await.map_err(|_| ())?;

    Ok(Token {
        access_token: json["access_token"].as_str().unwrap().to_string(),
        refresh_token: json["refresh_token"].as_str().unwrap().to_string(),
        expires_in: json["expires_in"].as_u64().unwrap_or(0),
    })
}

// ======================= CALLBACK SERVER =======================

fn wait_for_callback() -> String {
    let listener = TcpListener::bind("127.0.0.1:3000").expect("Failed to bind to localhost:3000");

    println!("Waiting for OAuth callback on http://localhost:3000...");

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();

        let mut buffer = [0; 2048];
        stream.read(&mut buffer).unwrap();

        let request = String::from_utf8_lossy(&buffer);

        if let Some(start) = request.find("GET /callback?code=") {
            let code_start = start + "GET /callback?code=".len();
            let code_end = request[code_start..]
                .find('&')
                .or_else(|| request[code_start..].find(' '))
                .unwrap();

            let code = &request[code_start..code_start + code_end];

            let response = "HTTP/1.1 200 OK\r\n\r\nYou can close this tab.";
            stream.write_all(response.as_bytes()).unwrap();

            return code.to_string();
        } else if let Some(start) = request.find("GET /callback?error=") {
            let error_start = start + "GET /callback?error=".len();
            let error_end = request[error_start..]
                .find('&')
                .or_else(|| request[error_start..].find(' '))
                .unwrap();
            let error = &request[error_start..error_start + error_end];

            let response = "HTTP/1.1 200 OK\r\n\r\nError: .".to_owned() + error;
            stream.write_all(response.as_bytes()).unwrap();
        }
    }

    panic!("Failed to receive OAuth callback");
}

// ======================= UTIL =======================

fn save_token(token: &Token) {
    let data = serde_json::to_string_pretty(token).unwrap();
    fs::write("token.json", data).unwrap();
}
