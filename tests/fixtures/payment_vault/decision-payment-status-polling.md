---
id: decision-payment-status-polling
type: Decision
title: Use HTTP polling for payment status
status: deprecated
valid_from: "2024-02-01"
valid_until: "2026-05-31"
scope:
  system: payments
  env: production
relations:
  - type: depends_on
    target: ./concept-payment-status.md
---
# Use HTTP Polling for Payment Status

## Context
When integrating with third-party payment gateways, services require real-time visibility into transaction transitions defined in [Payment Status Domain Model](./concept-payment-status.md).

## Decision
Payment services in production will poll the gateway HTTP status endpoint every 2 seconds until a terminal state (`Settled` or `Failed`) is reached, with a maximum timeout of 60 seconds.

## Status
Deprecated. Replaced after high concurrency led to thread pool exhaustion and gateway rate-limiting during peak loads.
