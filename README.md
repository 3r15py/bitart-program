# bitart

bitart is a Solana program for sending SOL with Unicode artwork embedded in the
same transaction. The artwork is rendered directly in program logs, so it can
be viewed through transaction explorers without creating an account, uploading
to external storage, or paying ongoing rent.

The program is intentionally small. It handles the on-chain transfer,
decompresses a bounded artwork payload, and emits 37 rows of Unicode Braille.
Image conversion and compression happen off-chain in the client.

## How it works

1. A client converts an image into a 148×148 one-bit contour map.
2. Each 2×4 pixel region becomes one Unicode Braille cell.
3. The resulting 74×37 grid is stored as 2,738 cell bytes.
4. The client compresses those bytes with the bitart LZW format.
5. One `send_bitart` instruction carries the compressed art and SOL amount.
6. The program transfers SOL through the System Program.
7. The program decompresses the art and writes all 37 rows to its logs.

Each decoded byte maps directly to `U+2800 + byte`. A zero byte therefore emits
the Unicode Braille blank rather than an ordinary space, preserving alignment
in explorers that collapse whitespace.

## Instruction interface

```text
send_bitart(lamports: u64, compressed_bitmap: Vec<u8>)
```

Accounts:

| Account | Access | Purpose |
| --- | --- | --- |
| `sender` | mutable signer | Pays the transfer and transaction fee |
| `recipient` | mutable | Receives the requested lamports |
| `system_program` | read-only | Executes the SOL transfer |

The recipient is intentionally unrestricted: any valid Solana account may
receive SOL. The sender must sign the transaction.

## Artwork format

- Canvas: 148×148 binary pixels
- Rendered output: 74 columns × 37 rows
- Decoded payload: exactly 2,738 bytes
- Compressed payload: at most 900 bytes
- Compression: bounded LZW with 9–12 bit codes
- LZW dictionary limit: 4,096 entries
- Code count: little-endian `u16` at the start of the compressed payload

Arranging the payload as neighboring Braille cells improves compression
compared with compressing the packed pixel raster, while producing identical
rendered artwork.

## Safety properties

- Compressed input is rejected above 900 bytes.
- Decompression cannot exceed the fixed 2,738-byte output limit.
- Invalid codes and malformed streams return program errors.
- No persistent account or user-provided executable account is created.
- The transfer uses Solana's System Program through CPI.
- No private key or deployment credential is required by the program API.

Clients should request a compute limit of 1,000,000 CU for the combined
decompression, transfer, Unicode conversion, and logging workload.

## Deployments

Program ID:

```text
BrzRKfKqtggYMQ7VkY7XmX3MgKUJwTVybuYCFeRKTxJ9
```

The same deterministic program address is deployed on both clusters:

- [Mainnet program](https://solscan.io/account/BrzRKfKqtggYMQ7VkY7XmX3MgKUJwTVybuYCFeRKTxJ9)
- [Mainnet deployment transaction](https://solscan.io/tx/4XpYSgaKEk1wHryNtctGmPYvfw51DtzKon8qDcB4wbJ8kKNxXJJBbXuBPdzLXXxPCNa4ZW1h7gkajqsJknsiEita)
- [Devnet program](https://solscan.io/account/BrzRKfKqtggYMQ7VkY7XmX3MgKUJwTVybuYCFeRKTxJ9?cluster=devnet)

The mainnet program is upgradeable. Its upgrade authority should be protected
as production infrastructure and never used by a browser client.

## Repository layout

```text
programs/postcard/src/lib.rs
programs/postcard/src/instructions/send_postcard.rs
programs/postcard/src/error.rs
programs/postcard/tests/postcard_sbf.rs
```

The package retains its original internal `postcard` name, while the public
instruction is `send_bitart`.

## Build and test

Requirements:

- Rust and Cargo
- Solana/Agave SBF build tools
- Anchor

```powershell
cargo build-sbf
cargo test -p postcard
```

The LiteSVM integration test loads the compiled SBF program, submits a complete
transaction, verifies the lamport transfer, checks that 37 art rows were
emitted, and confirms that logging was not truncated.

On Windows, a fresh Agave installation may require:

```powershell
cargo build-sbf --install-only --force-tools-install --tools-version v1.54
```

## Credential policy

Deployment wallets, seed phrases, private keys, environment files, generated
program keypairs, build output, and validator ledgers must never be committed.
The included `.gitignore` excludes the common local forms of these files.

