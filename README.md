# Counterpoint: A Chat Server

Counterpoint is a Rust-based chat server built to explore real-world backend architecture patterns, service
boundaries, and infrastructure development.

## Blog & Design Notes

Detailed design notes, architecture decisions, screenshots, logs, and sample events are documented here: TODO

## Requirements

- Rust (latest stable)
- Podman + podman-compose (or another compatible container runtime)
- Optional: [Client application](https://github.com/symphire/tune) (recommended for a full experience)

## Quick Start

If you just want a quick look at the project, running everything in containers is the easiest way to get started and
minimizes local environment setup.

The following commands will:
1. Generate a self-signed TLS certificate
2. Start the full container stack (infra + server)

```shell
cd counterpoint
bash dev-tools/create
podman compose up -d
```

Once the server logs "server started", it is ready to accept connections.

## Alternative Startup Methods

### Run the Server on Host (Development mode)

For development and debugging, you may prefer to run the server directly on the host while keeping infrastructure
services in containers.

Start the required infrastructure services:

```shell
podman compose up -d redis mysql kafka
```

Wait until all containers report `healthy` via `podman ps`, then start the server:


```shell
cargo run --package counterpoint --bin counterpoint
```

This mode provides faster iteration and more convenient debugging.

### Run Without a Client

If you do not have a client application available, you can use a one-off interactive demo tool that invokes the
server’s public APIs and prints serialized responses.

```shell
podman compose --profile tools --podman-run-args="-it" run --rm infra-demo
```
After the tool starts, you will see 13 informational log entries.
Wait until they finish printing, then press **Enter** to display the simulated chat history and exit.

## Platform Notes

The startup methods above are tested on **Arch Linux**.

They may require adjustments on other platforms (e.g. macOS or Windows).
If you encounter issues, please refer to the blog posts for screenshots, logs, and example outputs to verify
expected behavior.
