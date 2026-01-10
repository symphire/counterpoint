#!/bin/bash -x

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

CERT_DIR="$PROJECT_ROOT/certs"

mkdir -p "$CERT_DIR"
openssl req -x509 -newkey rsa:4096 -sha256 -days 365 -nodes \
  -keyout "$CERT_DIR/dev_key.pem" \
  -out "$CERT_DIR/dev_cert.pem" \
  -config "$SCRIPT_DIR/create_dev_cert.conf" \
  -extensions v3_req

chmod 600 "$CERT_DIR/dev_key.pem"