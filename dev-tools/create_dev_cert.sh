#!/bin/bash -x

mkdir certs
openssl req -x509 -newkey rsa:4096 -sha256 -days 365 -nodes \
  -keyout certs/dev_key.pem -out certs/dev_cert.pem \
  -config dev_cert.conf \
  -extensions v3_req