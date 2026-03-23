use tokio::sync::watch;
use tracing::info;

pub struct ShutdownCoordinator {
    tx: watch::Sender<bool>,
    rx: watch::Receiver<bool>,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Self { tx, rx }
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.rx.clone()
    }

    pub async fn wait_for_signal(&self) {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install ctrl+c handler");

        info!("ctrl+c received, shutting down");
        let _ = self.tx.send(true);
    }
}
