---
id: concept-payment-status
type: Concept
title: Payment Status Domain Model
status: stable
valid_from: "2024-01-01"
scope:
  system: payments
---
# Payment Status Domain Model

This note defines the core domain model and lifecycle state transitions for payment processing within the payments ecosystem.

## Lifecycle States
- **Initiated**: The transaction has been created by the client.
- **Pending**: Payment instruction dispatched to payment gateway; awaiting settlement.
- **Settled**: Funds successfully captured and verified.
- **Failed**: Terminal failure due to insufficient funds, processor error, or timeout.
- **Refunded**: Terminal reversal initiated post-settlement.

All payment services must adhere to this state machine baseline.
