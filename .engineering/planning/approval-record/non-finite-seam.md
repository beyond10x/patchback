---
format: aep.planning-md/3
id: approval-record:non-finite-seam
kind: approval-record
status: draft
title: Non-finite values are refused at the decoder seam, not in ESS
relations:
- decides: story:parameter-contracts
revision: 1
---
# Non-finite values are refused at the decoder seam, not in ESS

Approved by `b10x-chef-orchestrator` under the grant, 2026-10-23, for wave a2:

> ess answered your gap in https://github.com/beyond10x/ess/issues/676: by design. Decimal and
> Binary64 hold finite values only (ess docs/design/model-binary64.md:17; JSON has no NaN token).
> Approved for wave a2: refuse NaN/inf at your decoder/adapter seam and keep the Rust tests there.
> Drop the `non-finite` domain outcome or mark it external, and remove the 0.5 stand-in scenarios
> (non-finite.yaml:39-43, invalid-contract.yaml:7-10). No workaround is left to drop later.

## What changed

- `spec/domains/patch.yaml`: SetParameter's `non-finite` outcome stays, marked `external:`.
- `spec/scenarios/parameter-contracts-non-finite.yaml` lost its 0.5 stand-in step and is renamed
  `parameter-contracts-unknown-path.yaml`; story:parameter-contracts names it.
- `spec/scenarios/parameter-contracts-invalid-contract.yaml`: the `Gap (ESS 0.68.0)` note becomes a
  statement of where the refusal lives.
- The refusals are held by `crates/patchback-core/tests/parameter_contracts.rs`.
- No ESS pin move is pending for this.
