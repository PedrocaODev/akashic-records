---
id: incident-2026-05-status-timeout
type: Incident
title: Production Incident - Payment Status Polling Exhaustion
status: resolved
valid_from: "2026-05-15"
scope:
  system: payments
  env: production
relations:
  - type: caused_by
    target: ./decision-payment-status-polling.md
---
# Production Incident - Payment Status Polling Exhaustion

## Incident Description
On May 15, 2026 at 14:22 UTC, the production payment service became unresponsive to new checkout requests. Over 4,200 requests timed out across a 38-minute window.

## Root Cause
The architecture specified in [Use HTTP polling for payment status](./decision-payment-status-polling.md) enforced synchronous 2-second polling intervals. When downstream gateway response times degraded from 120ms to 2400ms, all 200 Tomcat worker threads became blocked waiting on polling responses, causing complete starvation.

## Resolution
Worker thread pools were restarted and rate-limit circuit breakers were deployed as temporary mitigation until event streaming architecture was completed.
