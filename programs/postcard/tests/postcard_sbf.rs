use std::{collections::HashMap, path::PathBuf};

use anchor_lang::{InstructionData, ToAccountMetas};
use litesvm::LiteSVM;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

#[test]
fn transfers_lamports_and_renders_braille_in_one_transaction() {
    let mut svm = LiteSVM::new();
    let program_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("deploy")
        .join("postcard.so");
    svm.add_program_from_file(postcard::ID, program_path)
        .unwrap();

    let sender = Keypair::new();
    let recipient = Keypair::new().pubkey();
    svm.airdrop(&sender.pubkey(), 1_000_000_000).unwrap();

    let mut bitmap = vec![0u8; 2_738];
    for row in 0..37 {
        bitmap[row * 74 + row * 2] = 0xff;
        bitmap[row * 74 + (73 - row * 2)] = 0xff;
    }
    let compressed_bitmap = lzw_encode(&bitmap);
    assert!(compressed_bitmap.len() < 800);

    let accounts = postcard::accounts::SendBitart {
        sender: sender.pubkey(),
        recipient,
        system_program: anchor_lang::system_program::ID,
    }
    .to_account_metas(None);
    let data = postcard::instruction::SendBitart {
        lamports: 1_000_000,
        compressed_bitmap,
    }
    .data();
    let instruction = Instruction {
        program_id: postcard::ID,
        accounts,
        data,
    };
    let message = Message::new_with_blockhash(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            instruction,
        ],
        Some(&sender.pubkey()),
        &svm.latest_blockhash(),
    );
    let transaction =
        VersionedTransaction::try_new(VersionedMessage::Legacy(message), &[&sender]).unwrap();

    let metadata = svm.send_transaction(transaction).unwrap();
    let recipient_account = svm.get_account(&recipient).unwrap();

    println!(
        "compute units consumed: {}",
        metadata.compute_units_consumed
    );
    assert_eq!(recipient_account.lamports, 1_000_000);
    assert!(metadata.logs.iter().any(|log| {
        log.chars()
            .any(|character| ('\u{2801}'..='\u{28ff}').contains(&character))
    }));
    assert!(!metadata.logs.iter().any(|log| log.contains("Instruction:")));
    assert_eq!(
        metadata
            .logs
            .iter()
            .filter(|log| log.starts_with("Program log: "))
            .count(),
        37
    );
    assert!(!metadata.logs.iter().any(|log| log == "Log truncated"));
    assert!(metadata.compute_units_consumed < 1_000_000);
}

fn lzw_encode(input: &[u8]) -> Vec<u8> {
    let mut dictionary = HashMap::<(u16, u8), u16>::new();
    let mut codes = Vec::<u16>::new();
    let mut next_code = 256u16;
    let mut prefix = u16::from(input[0]);

    for &byte in &input[1..] {
        if let Some(&code) = dictionary.get(&(prefix, byte)) {
            prefix = code;
        } else {
            codes.push(prefix);
            if next_code < 4_096 {
                dictionary.insert((prefix, byte), next_code);
                next_code += 1;
            }
            prefix = u16::from(byte);
        }
    }
    codes.push(prefix);

    let mut output = Vec::from((codes.len() as u16).to_le_bytes());
    let mut bit_buffer = 0u32;
    let mut bits_in_buffer = 0u8;
    for (index, code) in codes.into_iter().enumerate() {
        let width = match 255 + index {
            0..=511 => 9,
            512..=1_023 => 10,
            1_024..=2_047 => 11,
            _ => 12,
        };
        bit_buffer |= u32::from(code) << bits_in_buffer;
        bits_in_buffer += width;
        while bits_in_buffer >= 8 {
            output.push(bit_buffer as u8);
            bit_buffer >>= 8;
            bits_in_buffer -= 8;
        }
    }
    if bits_in_buffer > 0 {
        output.push(bit_buffer as u8);
    }
    output
}
