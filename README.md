# Cosmos Proposal Watcher 🚨

A monitoring tool that watches for new governance proposals on Cosmos-based chains and sends real-time alerts via Slack and/or Incident.io.

## Features

- **Multi-chain Support**: Monitor multiple Cosmos chains simultaneously
- **Status Tracking**: Track proposals through their lifecycle (Deposit → Voting → Passed/Rejected/Failed)
- **Alert Integration**: 
  - Slack notifications for team awareness
  - Incident.io integration for operational alerting with automatic resolution
- **Flexible Filtering**:
  - Filter by proposal status (monitor only specific states)
  - Filter by proposal type (e.g., only software upgrades)
- **Smart Deduplication**: Prevents duplicate alerts for the same proposal

## How It Works

The watcher:
1. Polls chain gRPC endpoints at regular intervals
2. Tracks proposal state transitions
3. Sends alerts when proposals change status
4. Automatically resolves previous alerts when proposals progress

### Alert Flow for Incident.io

- **Deposit Status**: Creates firing alert
- **Voting Status**: Resolves Deposit alert, creates new firing alert
- **Passed Status**: Resolves Voting alert, creates and immediately resolves Passed alert
- **Rejected/Failed Status**: Resolves Voting alert, creates urgent firing alert

## Installation

```bash
# Clone the repository
git clone https://github.com/your-org/cosmos-proposal-watcher.git
cd cosmos-proposal-watcher

# Build the binary
cargo build --release
```

## Configuration

### 1. Create Configuration File

Copy the example configuration and customize it:

```bash
cp chains.toml.example chains.toml
```

Edit `chains.toml` with your chain configurations, Slack webhook, and Incident.io settings.

### 2. Set Environment Variables (Optional)

You can override configuration values using environment variables:

```bash
# Slack webhook (overrides config file)
export SLACK_WEBHOOK_URL="https://hooks.slack.com/services/YOUR/WEBHOOK/URL"

# Incident.io token (overrides config file)
export INCIDENTIO_TOKEN="your-incident-io-api-token"
```

## Usage

### Run with Configuration File

```bash
./target/release/cosmos-proposal-watcher --config chains.toml
```

### Docker

```bash
# Build Docker image
docker build -t cosmos-proposal-watcher .

# Run with configuration file
docker run -v $(pwd)/chains.toml:/app/chains.toml \
  cosmos-proposal-watcher --config /app/chains.toml
```

## Development

### Run Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_get_proposals -- --show-output
```

### Local Development

```bash
# Run with debug logging
RUST_LOG=debug cargo run -- --config chains.toml

# Quick build and run
make
```

## Configuration Options

| Option | Description | Default |
|--------|-------------|---------|
| `grpc` | gRPC endpoint URL | Required |
| `chain_id` | Chain identifier | Required |
| `refresh` | Polling interval | 30s |
| `filter_status` | Proposal statuses to monitor (1-5) | All |
| `filter_type` | Message types to monitor | All |
| `slack.webhook_url` | Slack webhook URL | Optional |
| `incidentio.url` | Incident.io API endpoint | Optional |
| `incidentio.token` | Incident.io API token | Optional |

### Proposal Status Codes

- `1` - Deposit Period
- `2` - Voting Period
- `3` - Passed
- `4` - Rejected
- `5` - Failed

## Troubleshooting

### Debug Logging

Enable debug logs to troubleshoot issues:

```bash
RUST_LOG=debug ./cosmos-proposal-watcher --config chains.toml
```
