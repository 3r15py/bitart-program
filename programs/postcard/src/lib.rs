pub mod error;
pub mod instructions;

use anchor_lang::prelude::*;

pub use instructions::*;

declare_id!("BrzRKfKqtggYMQ7VkY7XmX3MgKUJwTVybuYCFeRKTxJ9");

#[program]
pub mod postcard {
    use super::*;

    pub fn send_bitart(
        ctx: Context<SendBitart>,
        lamports: u64,
        compressed_bitmap: Vec<u8>,
    ) -> Result<()> {
        crate::instructions::send_postcard::handle_send_bitart(ctx, lamports, compressed_bitmap)
    }
}
