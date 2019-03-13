set -e

# We create release rather than debug builds.
# Debug builds are very large and  applications
# may well exceed the maximum deployment package
# size for an AWS Lambda function.
docker run --rm \
  -v $(PWD):/code \
  -v $(echo $HOME)/.cargo/registry:/root/.cargo/registry \
  -v $(echo $HOME)/.cargo/git:/root/.cargo/git \
  softprops/lambda-rust

# Or instead of Docker, if we could locally link openssl...
#cargo build --release --target x86_64-unknown-linux-musl

FILENAME=rust.zip
zip -j $FILENAME ./target/x86_64-unknown-linux-musl/release/bootstrap
echo "Success! Upload $FILENAME to the AWS Lambda Console to deploy."
