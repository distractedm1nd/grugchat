use anyhow::{Context, Result};
use celestia_types::nmt::Namespace;
use ed25519_dalek::{ed25519::signature::Signer, SigningKey};
use keystore_rs::{KeyChain, KeyStore};
use reqwest::Client;
use serde_json::json;
use state::Message;
use std::{env, sync::Arc};
use tx::{Register, SendMessage, Signature, Transaction};

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
        "generate-key" => {
            let sk = keystore_rs::create_signing_key();
            let keychain = KeyChain;
            let res = keychain.add_signing_key(&sk);
            if res.is_err() {
                println!("Error: {}", res.err().unwrap());
            } else {
                println!("Public key: {}", hex::encode(sk.verifying_key().to_bytes()));
            }
        }
        "list-channels" => list_channels(&client, &server_url).await?,
        "read-channel" => {
            if args.len() < 3 {
                println!("Error: Channel name required");
                return Ok(());
            }
            read_channel(&client, &server_url, &args[2]).await?
        }
        "register-user" => {
            if args.len() < 3 {
                println!("Error: User ID required");
                return Ok(());
            }

            let keychain = KeyChain;
            let res = keychain.get_signing_key();
            if res.is_err() {
                println!("Error: {}", res.clone().err().unwrap());
            }

            register_user(&client, &server_url, &res.unwrap(), &args[2]).await?
        }
        "send-message" => {
            if args.len() < 3 {
                println!("Error: Channel and message required");
                return Ok(());
            }

            let keychain = KeyChain;
            let res = keychain.get_signing_key();
            if res.is_err() {
                println!("Error: {}", res.clone().err().unwrap());
            }
            // let key_hex = hex::encode(res.unwrap().verifying_key().to_bytes());

            send_message(&client, &server_url, &res.unwrap(), &args[2], &args[3]).await?
        }
        "start-fullnode" => {
            if args.len() < 4 {
                println!("Error: start height and namespace required");
                return Ok(());
            }
            let start_height = args[2]
                .parse::<u64>()
                .context("Failed to parse start height")?;

            let namespace_bytes =
                hex::decode(&args[3]).context("Failed to decode namespace hex")?;
            let namespace = Namespace::new_v0(namespace_bytes.as_slice())
                .context("Failed to create namespace")?;

            let fullnode = Arc::new(FullNode::new(namespace, start_height).await?);
            fullnode.start().await?;
            return Ok(());
        }
        _ => print_usage(),
    }

    Ok(())
}

fn print_usage() {
    println!("Usage:");
    println!("  grugchat generate-key");
    println!("  grugchat list-channels");
    println!("  grugchat read-channel <channel_name>");
    println!("  grugchat register-user <user_id>");
    println!("  grugchat send-message <channel> <message>");
    println!("  grugchat start-fullnode <start_height> <namespace_hex>");
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
    let messages: Option<Vec<Message>> = client
        .get(&format!("{}/channels/{}", server_url, channel))
        .send()
        .await?
        .json()
        .await?;

    if messages.is_none() {
        println!("Channel '{}' not found", channel);
        return Ok(());
    }

    println!("Messages in channel '{}':", channel);
    for msg in messages.unwrap() {
        println!("{}: {}", msg.user_id, msg.contents);
    }
    Ok(())
}
async fn register_user(
    client: &Client,
    server_url: &str,
    key: &SigningKey,
    id: &str,
) -> Result<()> {
    let public_key_bytes = key.clone().verifying_key().to_bytes().to_vec();
    let tx = Transaction::Register(Register {
        user: key.verifying_key().into(),
        id: id.to_string(),
        signature: Signature::new(Vec::new()),
    });

    let sig = key.sign(&bincode::serialize(&tx)?);
    let response = client
        .post(&format!("{}/register", server_url))
        .json(&json!({
            "public_key": public_key_bytes,
            "id": id,
            "signature": sig.to_bytes().to_vec(),
        }))
        .send()
        .await?;
    if response.status().is_success() {
        println!("User registration request sent successfully.");
    } else {
        let error_body = &response.text().await?;
        println!(
            "Failed to register user. Server responded with: {}",
            error_body
        );
    }
    Ok(())
}

async fn send_message(
    client: &Client,
    server_url: &str,
    key: &SigningKey,
    channel: &str,
    message: &str,
) -> Result<()> {
    let public_key_bytes = key.clone().verifying_key().to_bytes().to_vec();

    let tx = Transaction::SendMessage(SendMessage {
        user: key.clone().verifying_key().into(),
        channel: channel.to_string(),
        contents: message.to_string(),
        signature: Signature::new(Vec::new()),
    });

    let sig = key.clone().sign(&bincode::serialize(&tx)?);

    let response = client
        .post(&format!("{}/send", server_url))
        .json(&json!({
            "user": public_key_bytes,
            "channel": channel,
            "contents": message,
            "signature": sig.to_bytes().to_vec(),
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
