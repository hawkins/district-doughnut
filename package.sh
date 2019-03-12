set -e

# We create release rather than debug builds.
# Debug builds are very large and  applications
# may well exceed the maximum deployment package
# size for an AWS Lambda function.
docker run --rm -it -v "$(pwd)":/home/rust/src messense/rust-musl-cross:x86_64-musl cargo build --release

# Or instead of Docker, if we could locally link openssl...
#cargo build --release --target x86_64-unknown-linux-musl

FILENAME=rust.zip
zip -j $FILENAME ./target/x86_64-unknown-linux-musl/release/bootstrap
echo "Success! Upload $FILENAME to the AWS Lambda Console to deploy."
