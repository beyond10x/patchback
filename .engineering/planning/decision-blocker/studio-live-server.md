---
format: aep.planning-md/3
id: decision-blocker:studio-live-server
kind: decision-blocker
status: cleared
title: Studio live server needs ADR-011 in DESIGN.md
relations:
- blocks: story:studio-fit-live
revision: 4
transitions:
- {from: "open", to: "cleared", at: "2026-10-23T13:58:16Z", actor: "human:timo", revision: 3}
---
# Studio live server needs ADR-011

story:studio-fit-live adds a std-only server that streams fit frames to the studio. DESIGN.md § 3
forbids application frameworks and § 4.1 says no HTTP backend is necessary for the first release.
ADR-011 was to state that the studio is a developer tool outside the application dependency policy
and to settle the live server's scope.

## Cleared

DESIGN.md revision 0.3 carries ADR-011 (§ 14): the studio is outside the § 3 policy and the § 4.1
runtime rules, "it only draws", and "its live server is Rust `std` only"; § 3's table and § 4.1
say the same. Transport details stay open as OQ-24.

## Confirmed

`b10x-chef-orchestrator`, 2026-10-23, for Timo: its first answer on this item rested on a stale checkout
and is withdrawn; "ADR-011 stays as it is (DESIGN.md:523, 590cb82)". story:studio-fit-live is
scheduled when its turn comes, with no hold.
