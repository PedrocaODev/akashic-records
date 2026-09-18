---
id: runbook-payment-reconciliation
type: Runbook
title: Payment Reconciliation Runbook
status: stable
valid_from: "2026-06-15"
scope:
  system: payments
  env: production
relations:
  - type: depends_on
    target: ./decision-payment-status-events.md
---
# Payment Reconciliation Runbook

## Overview
Operational runbook for daily payment ledger reconciliation against Kafka event streams configured per [Use event-driven payment status](./decision-payment-status-events.md).

## Operational Steps
1. Verify consumer lag on topic `payments.status-events` via Prometheus metrics.
2. If consumer lag exceeds 500 messages, initiate worker auto-scaling.
3. Compare settlement ledger entries against Stripe daily settlement report.
4. Flag discrepancies greater than $0.01 for manual finance investigation.
