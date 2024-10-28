Build contract:
```shell
cargo build --manifest-path Cargo.toml --target wasm32-unknown-unknown --release
```

Deploy contract: 
```shell
near create omni_btc.testnet --useFaucet
near deploy omni_btc.testnet --wasm-file "./target/wasm32-unknown-unknown/release/omni_bitcoin.wasm" --init-function "new" --init-args '{"owner": "bitcoin_connector.testnet"}'
```

```shell
near create bitcoin_connector.testnet --useFaucet
near deploy bitcoin_connector.testnet --wasm-file "./target/wasm32-unknown-unknown/release/bitcoin_connector.wasm" --init-function "new" --init-args '{"omni_btc": "omni_btc.testnet"}'
```

```shell
near create btc_user.testnet --useFaucet
```
