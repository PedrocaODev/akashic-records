---
id: review-incident-timeout
type: Review
title: Post-Incident Review - Payment Status Polling Timeout
status: stable
valid_from: "2026-05-20"
scope:
  system: payments
  env: production
relations:
  - type: relates_to
    target: ./incident-2026-05-status-timeout.md
  - type: supported_by
    target: ./external-source-stripe.md
---
# Post-Incident Review - Payment Status Polling Timeout

## Executive Summary
On May 15, 2026, the payments service experienced severe thread pool exhaustion caused by high-concurrency polling of payment gateway APIs as recorded in [Production Incident - Payment Status Polling Exhaustion](./incident-2026-05-status-timeout.md).

## Root Cause Analysis
Synchronous 2-second HTTP polling created a cascading queue buildup during a partner processing delay. Upstream webhook SLAs documented in [Stripe Webhooks and Event Delivery SLA](./external-source-stripe.md) demonstrate that event-driven webhooks offer 99.99% delivery reliability within 300ms, proving that event ingestion is superior to HTTP polling.

## Corrective Actions
- Supersede HTTP polling with Kafka event streaming.
- Implement automated dead-letter queue reprocessing.
