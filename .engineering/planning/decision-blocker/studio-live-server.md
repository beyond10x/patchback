---
format: aep.planning-md/3
id: decision-blocker:studio-live-server
kind: decision-blocker
status: open
title: Studio live server needs ADR-011 in DESIGN.md
relations:
- blocks: story:studio-fit-live
revision: 1
---
# Studio live server needs ADR-011

story:studio-fit-live adds a std-only server that streams fit frames to the studio. DESIGN.md § 3
forbids application frameworks and § 4.1 says no HTTP backend is necessary for the first release.
ADR-011 (DESIGN.md rev 0.3, being written) is to state that the studio is a developer tool outside
the application dependency policy and to settle the live server's scope.

Cleared when DESIGN.md carries ADR-011 with the live server's scope.
