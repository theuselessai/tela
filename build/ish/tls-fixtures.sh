#!/bin/sh
# Ephemeral, test-only localhost certificates; never ship keys in a device archive.
set -eu
mkdir -p /out/tls-fixtures/empty-roots
cd /out/tls-fixtures
openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.pem -days 2 \
  -subj /CN=Tela-i586-test-only-CA -addext basicConstraints=critical,CA:TRUE
openssl req -newkey rsa:2048 -nodes -keyout server.key -out server.csr -subj /CN=localhost
printf 'subjectAltName=DNS:localhost\nbasicConstraints=critical,CA:FALSE\nextendedKeyUsage=serverAuth\n' > server.ext
openssl x509 -req -in server.csr -CA ca.pem -CAkey ca.key -CAcreateserial \
  -out server.pem -days 2 -extfile server.ext
openssl req -x509 -newkey rsa:2048 -nodes -keyout untrusted.key -out untrusted.pem \
  -days 2 -subj /CN=localhost -addext subjectAltName=DNS:localhost \
  -addext basicConstraints=critical,CA:FALSE
printf 'TEST ONLY: ephemeral fixture keys, not user credentials; DO NOT PACKAGE.\n' > README.txt
