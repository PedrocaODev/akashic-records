---
id: proposal-payment-grpc
type: Proposal
title: Use gRPC streaming for payment status
status: draft
valid_from: "2026-07-01"
scope:
  system: payments
  env: production
relations:
  - type: contradicts
    target: ./decision-payment-status-events.md
---
# Proposal: Use gRPC Streaming for Payment Status

## Abstract
This RFC proposes replacing Kafka event streams with direct bi-directional gRPC channels for sub-millisecond payment status notifications.

## Contradiction
This proposal directly contradicts the adopted architecture in [Use event-driven payment status](./decision-payment-status-events.md), which relies on asynchronous publish-subscribe rather than point-to-point RPC streaming.
