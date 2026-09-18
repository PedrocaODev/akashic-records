---
id: decision-payment-status-events
type: Decision
title: Use event-driven payment status
status: stable
valid_from: "2026-06-01"
scope:
  system: payments
  env: production
relations:
  - type: supersedes
    target: ./decision-payment-status-polling.md
    evidence: ./review-incident-timeout.md
  - type: supported_by
    target: ./review-incident-timeout.md
  - type: depends_on
    target: ./concept-payment-status.md
---
# Use Event-Driven Payment Status

## Context
Following the HTTP polling exhaustion incident reviewed in [Post-Incident Review - Payment Status Polling Timeout](./review-incident-timeout.md), synchronous polling against payment gateways has been decommissioned.

## Decision
Payment services in production will consume asynchronous webhook events via Apache Kafka topics (`payments.status-events`) rather than HTTP polling. All downstream consumers subscribe to partition-keyed events ordered by payment ID.

## Consequences
- Eliminates synchronous connection holding and polling storms.
- Provides sub-second notification latency for terminal state transitions.
- Governed by [Payment Status Domain Model](./concept-payment-status.md).
