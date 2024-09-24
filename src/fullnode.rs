use anyhow::{Context, Result};
use axum::{
    routing::{get, post},
    Router,
};
use celestia_rpc::{BlobClient, HeaderClient};
use celestia_types::{nmt::Namespace, Blob, TxConfig};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::spawn;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::time::{interval, Duration};

use crate::{state::State, tx::Transaction, webserver::*};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Batch(Vec<Transaction>);

const BATCH_INTERVAL: Duration = Duration::from_secs(3);

pub struct FullNode {
    da_client: celestia_rpc::Client,
    namespace: Namespace,
    start_height: u64,

    pub(crate) state: Arc<Mutex<State>>,
    pending_transactions: Arc<Mutex<Vec<Transaction>>>,

    genesis_sync_complete: Arc<AtomicBool>,
    sync_notify: Arc<Notify>,
}

impl TryFrom<&Blob> for Batch {
    type Error = anyhow::Error;

    fn try_from(value: &Blob) -> Result<Self, Self::Error> {
        match bincode::deserialize(&value.data) {
            Ok(batch) => Ok(batch),
            Err(_) => {
                let transaction: Transaction = bincode::deserialize(&value.data)
                    .context(format!("Failed to decode blob into Transaction: {value:?}"))?;

                Ok(Batch(vec![transaction]))
            }
        }
    }
}

impl FullNode {
    pub async fn new(namespace: Namespace, start_height: u64) -> Result<Self> {
        let da_client = celestia_rpc::Client::new("ws://localhost:26658", None)
            .await
            .context("Couldn't start Celestia client")?;

        Ok(FullNode {
            da_client,
            namespace,
            start_height,
            pending_transactions: Arc::new(Mutex::new(Vec::new())),
            state: Arc::new(Mutex::new(State::new())),
            genesis_sync_complete: Arc::new(AtomicBool::new(false)),
            sync_notify: Arc::new(Notify::new()),
        })
    }

    pub async fn start_server(self: Arc<Self>) -> Result<()> {
        let app = Router::new()
            .route("/channels", get(list_channels))
            .route("/channels/:channel", get(read_channel))
            .route("/register", post(register_user))
            .route("/send", post(send_message))
            .with_state(self.clone());

        let addr = "0.0.0.0:3000";
        println!("Server listening on {}", addr);
        axum::Server::bind(&addr.parse().unwrap())
            .serve(app.into_make_service())
            .await
            .context("Failed to start server")?;

        Ok(())
    }

    pub async fn queue_transaction(self: Arc<Self>, tx: Transaction) -> Result<()> {
        let mut pending_txs = self.pending_transactions.lock().await;
        pending_txs.push(tx);
        Ok(())
    }

    async fn post_pending_batch(self: Arc<Self>) -> Result<()> {
        let mut pending_txs = self.pending_transactions.lock().await;
        if pending_txs.is_empty() {
            return Ok(());
        }

        let batch = Batch(pending_txs.drain(..).collect());
        let encoded_batch = bincode::serialize(&batch)?;

        let blob = Blob::new(self.namespace, encoded_batch)?;
        BlobClient::blob_submit(&self.da_client, &[blob], TxConfig::default()).await?;

        println!("Batch posted with {} transactions", batch.0.len());
        Ok(())
    }

    async fn process_l1_block(self: Arc<Self>, blobs: Vec<Blob>) {
        let txs: Vec<Transaction> = blobs
            .into_iter()
            .flat_map(|blob| match Batch::try_from(&blob) {
                Ok(batch) => batch.0,
                Err(_) => Vec::new(),
            })
            .collect();

        let mut state = self.state.lock().await;
        txs.into_iter().for_each(|tx| match state.process_tx(tx) {
            Ok(_) => println!("Processed transaction"),
            Err(e) => eprintln!("Error processing tx: {}", e),
        });
    }

    async fn sync_from_genesis(self: Arc<Self>) -> Result<()> {
        let network_head = HeaderClient::header_network_head(&self.da_client).await?;
        let network_height = network_head.height();
        for height in self.start_height..network_height.value() {
            let response =
                BlobClient::blob_get_all(&self.da_client, height, &[self.namespace]).await?;
            if let Some(blobs) = response {
                self.clone().process_l1_block(blobs).await;
            }
        }
        self.genesis_sync_complete.store(true, Ordering::SeqCst);
        self.sync_notify.notify_waiters();
        Ok(())
    }

    pub async fn start_batch_posting(self: Arc<Self>) {
        let mut interval = interval(BATCH_INTERVAL);

        loop {
            interval.tick().await;
            if let Err(e) = self.clone().post_pending_batch().await {
                eprintln!("Error posting batch: {}", e);
            }
        }
    }

    async fn sync_incoming_blocks(self: Arc<Self>) -> Result<(), tokio::task::JoinError> {
        let (tx, mut rx) = mpsc::channel(100); // Adjust buffer size as needed

        // Start the subscription immediately
        let subscription_handle = spawn({
            let node = self.clone();
            async move {
                let mut blobsub = BlobClient::blob_subscribe(&node.da_client, node.namespace)
                    .await
                    .context("Failed to subscribe to app namespace")
                    .unwrap();

                while let Some(result) = blobsub.next().await {
                    match result {
                        Ok(blob_response) => {
                            if let Some(blobs) = blob_response.blobs {
                                if tx.send(blobs).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            println!("Error retrieving blobs from DA layer: {}", e);
                        }
                    }
                }
            }
        });

        // Wait for genesis sync to complete before processing incoming blocks
        self.sync_notify.notified().await;

        // Process incoming blocks
        while let Some(blobs) = rx.recv().await {
            self.clone().process_l1_block(blobs).await;
        }

        subscription_handle.await
    }

    pub async fn start_sync(self: Arc<Self>) -> Result<()> {
        let genesis_sync = spawn({
            let node = self.clone();
            async move { node.sync_from_genesis().await }
        });

        let incoming_sync = spawn({
            let node = self.clone();
            async move { node.sync_incoming_blocks().await }
        });

        let _ = tokio::try_join!(genesis_sync, incoming_sync)?;

        Ok(())
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        let sync_handle = spawn({
            let node = self.clone();
            async move { node.start_sync().await }
        });

        let batch_posting_handle = spawn({
            let node = self.clone();
            async move { node.start_batch_posting().await }
        });

        let server_handle = spawn({
            let node = self.clone();
            async move { node.start_server().await }
        });

        let _ = tokio::try_join!(sync_handle, batch_posting_handle, server_handle)?;

        Ok(())
    }
}
