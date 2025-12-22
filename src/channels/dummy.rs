use crate::core::error::Result;
use crate::core::traits::MessageHandler;
use crate::core::types::Message;
use async_trait::async_trait;

/// A dummy message handler that does nothing, used for cleanup to break reference cycles
pub struct DummyMessageHandler;

#[async_trait]
impl MessageHandler for DummyMessageHandler {
    async fn handle_message(&self, _message: Message) -> Result<()> {
        // Do nothing - this is a no-op handler
        Ok(())
    }
}