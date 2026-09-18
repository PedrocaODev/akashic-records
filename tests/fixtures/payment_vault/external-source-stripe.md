---
id: external-source-stripe
type: Source
title: Stripe Webhooks and Event Delivery SLA
status: stable
valid_from: "2025-01-01"
scope:
  system: payments
---
# Stripe Webhooks and Event Delivery SLA

## Source Provenance
Documenting the upstream partner specification and SLA contract for payment lifecycle webhooks.

## SLA Guarantees
- 99.99% event delivery reliability.
- Latency P95 under 300ms from terminal transaction settlement.
- Automatic retry schedule with exponential backoff over 72 hours.
