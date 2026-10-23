//! Async completion integration and bounded headless event driving.
use super::*;

impl Engine {
    pub(super) async fn dispatch_input_action(&self, action: serde_json::Value) -> Result<bool> {
        if self.reduce_and_process(action).await? {
            return Ok(true);
        }
        // Input reducers can schedule Promise continuations which start I/O.
        // Drive them now: there may be no network/timer event yet to wake run().
        self.finish_async_actions().await?;
        Ok(self.check_quit().await)
    }
    pub(super) async fn crypto_ready(&self) {
        if let Some(crypto) = &self.crypto {
            crypto.notified().await;
        } else {
            std::future::pending::<()>().await;
        }
    }
    pub(super) async fn handle_crypto_completion(&self) -> Result<bool> {
        if let Some(crypto) = &self.crypto {
            self.ctx
                .with(|ctx| crate::native::crypto::deliver_crypto(&ctx, crypto))
                .await?;
        }
        self.finish_async_actions().await?;
        Ok(self.check_quit().await)
    }
    pub(super) async fn finish_async_actions(&self) -> Result<()> {
        // Promise continuations may dispatch after a transport callback returns.
        // Drain them BEFORE rendering, not only before pumping Promise jobs.
        for _ in 0..4 {
            self.execute_js_jobs().await;
            let actions = self.drain_dispatch_queue().await?;
            if actions.is_empty() {
                break;
            }
            for action in actions {
                self.reduce_and_process(action).await?;
            }
        }
        Ok(())
    }

    /// Advance an embedded/headless app without taking over a terminal.
    /// Do not call concurrently with run(); that loop owns the action receiver.
    /// A true result means the application requested quit.
    pub async fn pump_events(&self) -> Result<bool> {
        let mut receiver = self
            .action_rx
            .try_lock()
            .map_err(|_| anyhow!("engine event loop is already active"))?;
        for _ in 0..64 {
            let Ok(action) = receiver.try_recv() else {
                break;
            };
            self.handle_internal_action(action).await?;
        }
        self.drain_native_completions().await;
        self.handle_crypto_completion().await?;
        self.finish_async_actions().await?;
        Ok(self.check_quit().await)
    }
}
