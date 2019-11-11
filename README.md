# Fast Foodie

An AWS lambda function to scape fast food menu and look for new items

Sends a notification to an SNS topic for both newly released and discarded items

## Setup

For local compilation: (YMMV)

```sh
rustup target add x86_64-unknown-linux-musl
brew install filosottile/musl-cross/musl-cross
mkdir .cargo

echo '[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"' > .cargo/config'
```

Otherwise, install Docker

## Building and deploying

For compiling in Docker and deploying:

```sh
# Edit upload.sh first to change parameters to suit you
# Then run upload.sh
./upload.sh
```
