use anchor_lang::prelude::*;

#[error_code]
pub enum PostcardError {
    #[msg("The compressed bitmap exceeds 900 bytes")]
    CompressedPayloadTooLarge,
    #[msg("The payload is not valid LZW-compressed bitmap data")]
    InvalidCompressedBitmap,
    #[msg("The decompressed bitmap is not exactly 148x148 pixels")]
    InvalidBitmapLength,
}
