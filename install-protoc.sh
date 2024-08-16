#!/bin/bash

# Get the latest protoc version
PROTOC_VERSION=$(curl -s https://api.github.com/repos/protocolbuffers/protobuf/releases/latest | grep -oP '"tag_name": "\K(.*)(?=")')
PROTOC_VERSION=${PROTOC_VERSION#v}

# Download the protoc binary
curl -LO "https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/protoc-${PROTOC_VERSION}-linux-x86_64.zip"

# Unzip the downloaded file to /usr/local
sudo unzip "protoc-${PROTOC_VERSION}-linux-x86_64.zip" -d /usr/local

# Remove the downloaded zip file
rm "protoc-${PROTOC_VERSION}-linux-x86_64.zip"

# Verify the installation
protoc --version

echo "protoc ${PROTOC_VERSION} has been installed successfully."
