use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use state::Message;
use std::{env, sync::Arc};

mod fullnode;
mod state;
mod tx;
mod webserver;

use crate::fullnode::FullNode;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let client = Client::new();
    let server_url =
        env::var("GRUGCHAT_SERVER_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    match args[1].as_str() {
        "list-channels" => list_channels(&client, &server_url).await?,
        "read-channel" => {
            if args.len() < 3 {
                println!("Error: Channel name required");
                return Ok(());
            }
            read_channel(&client, &server_url, &args[2]).await?
        }
        "register-user" => {
            if args.len() < 4 {
                println!("Error: Public key and user ID required");
                return Ok(());
            }
            register_user(&client, &server_url, &args[2], &args[3]).await?
        }
        "send-message" => {
            if args.len() < 5 {
                println!("Error: Public key, channel, and message required");
                return Ok(());
            }
            send_message(&client, &server_url, &args[2], &args[3], &args[4]).await?
        }
        "start-fullnode" => {
            let fullnode = Arc::new(FullNode::new().await?);
            fullnode.start().await?;
            return Ok(());
        }
        _ => print_usage(),
    }

    Ok(())
}

fn print_usage() {
    println!("Usage:");
    println!("  grugchat list-channels");
    println!("  grugchat read-channel <channel_name>");
    println!("  grugchat register-user <public_key_hex> <user_id>");
    println!("  grugchat send-message <public_key_hex> <channel> <message>");
    println!("  grugchat start-fullnode");
}

async fn list_channels(client: &Client, server_url: &str) -> Result<()> {
    let channels: Vec<String> = client
        .get(&format!("{}/channels", server_url))
        .send()
        .await?
        .json()
        .await?;

    println!("Channels:");
    for channel in channels {
        println!("- {}", channel);
    }
    Ok(())
}

async fn read_channel(client: &Client, server_url: &str, channel: &str) -> Result<()> {
    let messages: Vec<Message> = client
        .get(&format!("{}/channels/{}", server_url, channel))
        .send()
        .await?
        .json()
        .await?;

    println!("Messages in channel '{}':", channel);
    for msg in messages {
        println!("{}: {}", msg.user_id, msg.contents);
    }
    Ok(())
}

async fn register_user(
    client: &Client,
    server_url: &str,
    public_key_hex: &str,
    id: &str,
) -> Result<()> {
    let public_key = hex::decode(public_key_hex).context("Failed to decode public key")?;

    let response = client
        .post(&format!("{}/register", server_url))
        .json(&json!({
            "public_key": public_key,
            "id": id
        }))
        .send()
        .await?;

    if response.status().is_success() {
        println!("User registration request sent successfully.");
    } else {
        println!(
            "Failed to register user. Server responded with: {}",
            response.status()
        );
    }
    Ok(())
}

async fn send_message(
    client: &Client,
    server_url: &str,
    public_key_hex: &str,
    channel: &str,
    message: &str,
) -> Result<()> {
    let public_key = hex::decode(public_key_hex).context("Failed to decode public key")?;

    let response = client
        .post(&format!("{}/send", server_url))
        .json(&json!({
            "user": public_key,
            "channel": channel,
            "contents": message
        }))
        .send()
        .await?;

    if response.status().is_success() {
        println!("Message sent successfully.");
    } else {
        println!(
            "Failed to send message. Server responded with: {}",
            response.status()
        );
    }
    Ok(())
}
