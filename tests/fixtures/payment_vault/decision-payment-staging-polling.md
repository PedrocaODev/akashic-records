---
id: decision-payment-staging-polling
type: Decision
title: Use mock HTTP polling in staging
status: stable
valid_from: "2026-06-01"
scope:
  system: payments
  env: staging
relations:
  - type: depends_on
    target: ./concept-payment-status.md
---
# Use Mock HTTP Polling in Staging

## Context
In staging environments, external event streaming infrastructure (Kafka cluster) is cost-prohibitive and unnecessary for end-to-end integration testing of [Payment Status Domain Model](./concept-payment-status.md).

## Decision
The staging environment will continue to use mock HTTP polling against the sandbox gateway container. This policy applies strictly to `env: staging`. Production remains exclusively event-driven.
