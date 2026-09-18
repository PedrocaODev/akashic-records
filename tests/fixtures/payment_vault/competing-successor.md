---
id: competing-successor
type: Decision
title: Alternative Webhook Payment Status
status: stable
valid_from: "2026-06-01"
scope:
  system: payments
  env: production
relations:
  - type: supersedes
    target: ./decision-payment-status-polling.md
    evidence: ./review-incident-timeout.md
---
# Alternative Webhook Payment Status

## Decision
An alternative proposal to supersede HTTP polling by using direct point-to-point HTTPS webhooks instead of an Apache Kafka cluster.

## Competing Status
Competes directly with `decision-payment-status-events` without an acyclic supersession between them. Supported by evidence in [Post-Incident Review - Payment Status Polling Timeout](./review-incident-timeout.md).
