# District Doughnut

A lambda function to scape the District Doughnut website and look for new flavors

## Setup

```sh
rustup target add x86_64-unknown-linux-musl
brew install filosottile/musl-cross/musl-cross
mkdir .cargo

echo '[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"' > .cargo/config'
```

## Building

```sh
OPENSSL_DIR=$(brew --prefix openssl) cargo build --release --target x86_64-unknown-linux-musl
zip -j rust.zip ./target/x86_64-unknown-linux-musl/release/bootstrap
```
