# Laminar Backend

This workspace contains the Rust backend for Laminar.

Current status:
- Phase 1 scaffold only
- No protocol logic yet
- No indexer logic yet
- No quote engine yet
- No keeper logic yet

Planned binaries:
- `api`
- `indexer`
- `keeper`
- `executor`

Planned shared crates:
- `laminar-core`
- `laminar-chain`
- `laminar-store`
- `laminar-quote`
- `laminar-tx`
- `laminar-api`
- `laminar-indexer`
- `laminar-keeper`
- `laminar-telemetry`
- `laminar-config`

## Local setup

1. Start local infra:
```bash
docker compose -f backend/docker/compose.yml up -d
