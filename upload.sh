set -e

# We create release rather than debug builds.
# Debug builds are very large and applications
# may well exceed the maximum deployment package
# size for an AWS Lambda function.
docker run --rm \
  -v $(PWD):/code \
  -v $(echo $HOME)/.cargo/registry:/root/.cargo/registry \
  -v $(echo $HOME)/.cargo/git:/root/.cargo/git \
  softprops/lambda-rust

# Or instead of Docker, if we could locally link openssl...
#cargo build --release --target x86_64-unknown-linux-musl

# Configure deployment here
BUCKET=code-archive
STACK_NAME=fast-foodie
PACKAGED_TEMPLATE=packaged.yaml
TABLE_NAME=fast-foodie
PHONE_NUMBER=15551234567
MENU_URL="https://www.districtdoughnut.com/doughnuts"
RESTAURANT="District-Doughtnut"

# Package will upload the code to the S3 bucket
aws cloudformation package \
  --template-file template.yaml \
  --s3-bucket $BUCKET \
  --output-template-file $PACKAGED_TEMPLATE

echo 'Deploying now...'

# Finally deploy the stack now
aws cloudformation deploy \
  --template-file $PACKAGED_TEMPLATE \
  --stack-name $STACK_NAME \
  --capabilities CAPABILITY_IAM \
  --parameter-overrides \
  TableName=$TABLE_NAME \
  PhoneNumber=$PHONE_NUMBER \
  Menu=$MENU_URL \
  Restaurant=$RESTAURANT

