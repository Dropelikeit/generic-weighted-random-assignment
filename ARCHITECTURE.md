# Architecture

## Overview

The project follows a layered architecture with clear separation of concerns:

```
┌──────────────────────────────────┐
│          External Clients        │
│     (PHP, Python, curl, etc.)    │
└──────────┬───────────┬───────────┘
           │           │
     ┌─────▼─────┐ ┌───▼───┐
     │  REST API  │ │  CLI  │
     │  (axum)    │ │(clap) │
     └─────┬──────┘ └───┬───┘
           │            │
     ┌─────▼────────────▼──────┐
     │       Engine Layer      │
     │  (Orchestration +       │
     │   Validation)           │
     └─────────┬───────────────┘
               │
     ┌─────────▼───────────────┐
     │       Core Layer        │
     │  ┌──────────────────┐   │
     │  │    Algorithm      │   │
     │  │  (Derangement +   │   │
     │  │   Weighted Select)│   │
     │  └────────┬─────────┘   │
     │  ┌────────▼─────────┐   │
     │  │ Penalty Strategies│   │
     │  │ (Linear, Exp,     │   │
     │  │  Threshold)       │   │
     │  └──────────────────┘   │
     │  ┌──────────────────┐   │
     │  │  Domain Models   │   │
     │  └──────────────────┘   │
     └─────────────────────────┘
               │
     ┌─────────▼───────────────┐
     │    Infrastructure       │
     │  (Config, Logging)      │
     └─────────────────────────┘
```

## Module Structure

### `core/` - Domain Core

The core module contains no external dependencies beyond `rand` and `serde`. It is the heart of the system.

#### `core/models.rs`

Domain types shared across the system:

- `HistoricalPairing` - A record of a past giver-receiver pairing with occurrence count.
- `Assignment` - A single giver-to-receiver assignment.
- `AssignmentResult` - The collection of all assignments for a run.
- `AssignmentConfig` - Configuration combining participants, history, strategy, and seed.

#### `core/penalty.rs`

The `PenaltyStrategy` trait and its implementations:

- **`LinearPenalty`** - `adjusted_weight = base_weight / (1 + penalty_factor * count)`
- **`ExponentialPenalty`** - `adjusted_weight = base_weight * decay_rate^count`
- **`ThresholdPenalty`** - Full weight below threshold, reduced weight above.

The trait is `Send + Sync + Debug`, enabling use across async contexts.

#### `core/algorithm.rs`

The assignment algorithm:

1. **History map construction** - O(1) lookup for pairing counts.
2. **Weighted candidate selection** - For each giver, build a weighted list of receivers.
3. **Look-ahead logic** - Prevents dead-ends by checking if taking a receiver would strand another giver.
4. **Retry mechanism** - Re-shuffles and retries if a dead-end occurs (max 100 attempts).
5. **Deterministic seeding** - Optional seed for reproducible output.

### `engine/` - Orchestration

The engine layer:

- Validates inputs (non-empty participants, valid history references, non-blank names).
- Logs the generation process using `tracing`.
- Delegates to the core algorithm.
- Maps algorithm errors to engine-level errors.

### `api/` - REST API

Built with `axum`:

- `POST /assignments/generate` - Main endpoint for generating assignments.
- `GET /health` - Health check endpoint.
- CORS support via `tower-http`.
- Request tracing via `tower-http::trace`.

### `cli/` - Command Line Interface

Built with `clap`:

- `generate` subcommand with flags for participants, history file, strategy, and seed.
- Outputs JSON to stdout.
- Reads history from JSON files.

### `infra/` - Infrastructure

- `config.rs` - Server configuration (host, port).
- `logging.rs` - Tracing subscriber initialization with `RUST_LOG` support.

## Design Decisions

### Why derangement instead of simple permutation?

A derangement guarantees that no participant is assigned to themselves. This is a fundamental requirement for use cases like gift exchanges, task rotation, or mentoring pairing.

### Why retry-based approach?

The algorithm uses a greedy approach with look-ahead. While not guaranteed to succeed on the first attempt, the look-ahead prevents most dead-ends. For any practical number of participants, the algorithm succeeds within a few attempts.

### Why trait-based penalty strategies?

The `PenaltyStrategy` trait enables:
- Easy extension with new strategies without modifying existing code (Open/Closed Principle).
- Clean testing of each strategy in isolation.
- Runtime strategy selection via the API.

### Why separate engine and core layers?

- The **core** contains pure algorithm logic with no validation overhead.
- The **engine** handles validation, logging, and orchestration.
- This separation enables unit testing of the algorithm without validation noise.
