<p align="center">
  <img src="assets/readme/logo.svg" width="128" height="128" alt="Astrum Logo">
</p>

<h1 align="center">Astrum</h1>

<p align="center">
  A local-first AI chat application
</p>

## About

> It's still a bit rough around the edges and is still missing some features such as: chat titles, thinking, web search, markdown and message cancellation.

## Features

- **Multi-Provider Support** — Connect to OpenAI, Anthropic, or Ollama with a unified interface
- **Local-First** — All conversations persist in a local SQLite database

- - -

<p align="center">
  <img src="assets/readme/demo.png" alt="Astrum Demo" width="600">
</p>

- - -

## Installation

### Prerequisites

- Rust toolchain (1.75+)

### Build from Source

```bash
git clone https://github.com/your-username/astrum.git
cd astrum
cargo build --release
```

### Run

```bash
cargo run --release
```
