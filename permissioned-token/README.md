# Permissioned token

Implement a permissioned token, an authority can update permissions to allow certain owners to send and receive the token by adding them to the `PermissionRegistry`

```rust
pub struct Permission {
    pub owner: Pubkey,
    pub allowed_send: bool,
    pub allowed_receive: bool,
    pub expire_at: i64,
}
```

It is a fork of the transfer hook example
https://github.com/solana-labs/solana-program-library/tree/master/token/transfer-hook-example

On top of it we add necessary methods to add and update permissions

Test with `cargo test-bpf`
