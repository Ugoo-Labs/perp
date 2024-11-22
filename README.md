cargo build --release --target wasm32-unknown-unknown --package perp

candid-extractor target/wasm32-unknown-unknown/release/perp.wasm > src/perp/perp.did