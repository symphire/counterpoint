# ServerOxide
A Rust server.

## Requirements

- Rust (latest stable)
- `wscat` for WebSocket testing: `npm install -g wscat`
- Place self-signed certificate files at:
  - `certs/dev_cert.pem`
  - `certs/dev_key.pem`

## Quick Start

### 1. Run the Server

```bash
cargo run --bin counterpoint
```

### 2. Try the Basics

Run the manual test script to try:

- Captcha generation
- User signup & login

```bash
cd dev-tools
bash manual_dev_test.sh
```

### 3. WebSocket Chat Demo

Open four terminal tabs and run:

```bash
wscat -c wss://127.0.0.1:8443/api/v1/chat \
  --no-check \
  -H 'Authorization: Bearer fake-access-token:testuser0'
```
Replace `testuser0` with `testuser1`, `testuser2`, and `testuser3` for additional users.

Send a message: `{"type":"send","payload":{"conversation_id":"00000000-0000-0000-0000-000000000000","content":"Hello"}}`

#### Message Routing Behavior

- **testuser0 ↔ testuser1**: private 1-1 chat
- **testuser2 → testuser0 & testuser1**: group chat simulation
- **testuser3**: messages are dropped (simulates error case)