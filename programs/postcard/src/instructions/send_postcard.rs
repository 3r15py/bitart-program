use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

use crate::error::PostcardError;

const CELL_WIDTH: usize = 74;
const CELL_HEIGHT: usize = 37;
const ART_BYTES: usize = CELL_WIDTH * CELL_HEIGHT;
const MAX_COMPRESSED_BYTES: usize = 900;

#[derive(Accounts)]
pub struct SendBitart<'info> {
    #[account(mut)]
    pub sender: Signer<'info>,
    /// CHECK: The recipient can be any system account or wallet address.
    #[account(mut)]
    pub recipient: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn handle_send_bitart(
    ctx: Context<SendBitart>,
    lamports: u64,
    compressed_bitmap: Vec<u8>,
) -> Result<()> {
    require!(
        compressed_bitmap.len() <= MAX_COMPRESSED_BYTES,
        PostcardError::CompressedPayloadTooLarge
    );

    let art = decode_lzw(&compressed_bitmap, ART_BYTES)
        .ok_or_else(|| error!(PostcardError::InvalidCompressedBitmap))?;
    require!(art.len() == ART_BYTES, PostcardError::InvalidBitmapLength);

    if lamports > 0 {
        let accounts = Transfer {
            from: ctx.accounts.sender.to_account_info(),
            to: ctx.accounts.recipient.to_account_info(),
        };
        transfer(
            CpiContext::new(anchor_lang::system_program::ID, accounts),
            lamports,
        )?;
    }

    for row in art.chunks_exact(CELL_WIDTH).take(CELL_HEIGHT) {
        let mut line = String::with_capacity(CELL_WIDTH * 3);
        for &dots in row {
            line.push(char::from_u32(0x2800 + u32::from(dots)).unwrap());
        }
        msg!("{}", line);
    }

    Ok(())
}

fn decode_lzw(encoded: &[u8], output_limit: usize) -> Option<Vec<u8>> {
    if encoded.len() < 3 {
        return None;
    }

    let code_count = usize::from(u16::from_le_bytes([encoded[0], encoded[1]]));
    if code_count == 0 || code_count > output_limit {
        return None;
    }

    let mut input_offset = 2usize;
    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0u8;
    let mut prefixes: Vec<u16> = Vec::with_capacity(code_count.saturating_sub(1));
    let mut suffixes: Vec<u8> = Vec::with_capacity(code_count.saturating_sub(1));
    let mut scratch = Vec::with_capacity(256);
    let mut output = Vec::with_capacity(output_limit);

    let first = read_code(
        encoded,
        &mut input_offset,
        &mut bit_buffer,
        &mut bits_in_buffer,
        9,
    )?;
    if first >= 256 {
        return None;
    }
    output.push(first as u8);
    let mut previous = first;

    for code_index in 1..code_count {
        let width = code_width(code_index);
        let code = read_code(
            encoded,
            &mut input_offset,
            &mut bit_buffer,
            &mut bits_in_buffer,
            width,
        )?;
        let next_code = 256 + prefixes.len() as u16;

        scratch.clear();
        if code < next_code {
            expand_code(code, &prefixes, &suffixes, &mut scratch)?;
            scratch.reverse();
        } else if code == next_code {
            expand_code(previous, &prefixes, &suffixes, &mut scratch)?;
            scratch.reverse();
            let first_byte = *scratch.first()?;
            scratch.push(first_byte);
        } else {
            return None;
        }

        let first_byte = *scratch.first()?;
        if output.len() + scratch.len() > output_limit {
            return None;
        }
        output.extend_from_slice(&scratch);

        if prefixes.len() < 3_840 {
            prefixes.push(previous);
            suffixes.push(first_byte);
        }
        previous = code;
    }

    Some(output)
}

fn expand_code(
    mut code: u16,
    prefixes: &[u16],
    suffixes: &[u8],
    output: &mut Vec<u8>,
) -> Option<()> {
    let mut depth = 0usize;
    while code >= 256 {
        let index = usize::from(code - 256);
        output.push(*suffixes.get(index)?);
        code = *prefixes.get(index)?;
        depth += 1;
        if depth > 4_096 {
            return None;
        }
    }
    output.push(code as u8);
    Some(())
}

fn read_code(
    input: &[u8],
    input_offset: &mut usize,
    bit_buffer: &mut u32,
    bits_in_buffer: &mut u8,
    width: u8,
) -> Option<u16> {
    while *bits_in_buffer < width {
        *bit_buffer |= u32::from(*input.get(*input_offset)?) << *bits_in_buffer;
        *input_offset += 1;
        *bits_in_buffer += 8;
    }

    let mask = (1u32 << width) - 1;
    let code = (*bit_buffer & mask) as u16;
    *bit_buffer >>= width;
    *bits_in_buffer -= width;
    Some(code)
}

fn code_width(code_index: usize) -> u8 {
    match 255 + code_index {
        0..=511 => 9,
        512..=1_023 => 10,
        1_024..=2_047 => 11,
        _ => 12,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn lzw_round_trip_matches_browser_format() {
        let input: Vec<u8> = (0..2_048)
            .map(|index| {
                if index % 61 < 17 {
                    (index % 251) as u8
                } else {
                    0
                }
            })
            .collect();
        let encoded = encode_for_test(&input);

        assert_eq!(decode_lzw(&encoded, input.len()), Some(input));
    }

    #[test]
    fn lzw_rejects_output_over_limit() {
        let encoded = encode_for_test(&vec![0; 2_048]);

        assert!(decode_lzw(&encoded, 1_024).is_none());
    }

    fn encode_for_test(input: &[u8]) -> Vec<u8> {
        let mut dictionary = HashMap::<(u16, u8), u16>::new();
        let mut codes = Vec::<u16>::new();
        let mut next_code = 256u16;
        let mut prefix = u16::from(input[0]);

        for &byte in &input[1..] {
            if let Some(&code) = dictionary.get(&(prefix, byte)) {
                prefix = code;
            } else {
                codes.push(prefix);
                dictionary.insert((prefix, byte), next_code);
                next_code += 1;
                prefix = u16::from(byte);
            }
        }
        codes.push(prefix);

        let mut encoded = Vec::from((codes.len() as u16).to_le_bytes());
        let mut bit_buffer = 0u32;
        let mut bits_in_buffer = 0u8;
        for (index, code) in codes.into_iter().enumerate() {
            let width = code_width(index);
            bit_buffer |= u32::from(code) << bits_in_buffer;
            bits_in_buffer += width;
            while bits_in_buffer >= 8 {
                encoded.push(bit_buffer as u8);
                bit_buffer >>= 8;
                bits_in_buffer -= 8;
            }
        }
        if bits_in_buffer > 0 {
            encoded.push(bit_buffer as u8);
        }
        encoded
    }
}
