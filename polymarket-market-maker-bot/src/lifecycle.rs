use tokio::signal;
use tokio::time::{sleep, Duration};
use std::sync::Arc;

pub struct Lifecycle {
    sync_interval: u64,
    startup_callback: Option<Arc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>>,
    sync_callback: Option<Arc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>>,
    shutdown_callback: Option<Arc<dyn Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>>,
}

impl Lifecycle {
    pub fn new() -> Self {
        Self {
            sync_interval: 30,
            startup_callback: None,
            sync_callback: None,
            shutdown_callback: None,
        }
    }

    pub fn on_startup<F>(&mut self, callback: F)
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync + 'static,
    {
        self.startup_callback = Some(Arc::new(callback));
    }

    pub fn every<F>(&mut self, interval: u64, callback: F)
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync + 'static,
    {
        self.sync_interval = interval;
        self.sync_callback = Some(Arc::new(callback));
    }

    pub fn on_shutdown<F>(&mut self, callback: F)
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync + 'static,
    {
        self.shutdown_callback = Some(Arc::new(callback));
    }

    pub async fn run(&self) {
        log::info!("Initializing keeper lifecycle...");

        if let Some(ref startup) = self.startup_callback {
            log::info!("Executing keeper startup logic...");
            startup().await;
            log::info!("Startup complete!");
            sleep(Duration::from_secs(5)).await;
        }

        let sync_callback = self.sync_callback.clone();
        let sync_interval = self.sync_interval;

        let sync_handle = tokio::spawn(async move {
            if let Some(ref sync) = sync_callback {
                loop {
                    sync().await;
                    sleep(Duration::from_secs(sync_interval)).await;
                }
            }
        });

        tokio::select! {
            _ = signal::ctrl_c() => {
                log::info!("Keeper received SIGINT/SIGTERM signal, will terminate gracefully");
            }
            _ = sync_handle => {
                log::info!("Sync handle terminated");
            }
        }

        if let Some(ref shutdown) = self.shutdown_callback {
            log::info!("Executing keeper shutdown logic...");
            shutdown().await;
            log::info!("Shutdown logic finished");
        }

        log::info!("Keeper terminated");
    }
}

