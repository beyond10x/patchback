---
format: aep.planning-md/3
id: approval-record:landing-page
kind: approval-record
status: draft
title: Animated landing page and code highlighting approved
relations:
- decides: vision:patchback
revision: 1
---
# Landing page and code highlighting approved

Operator, 2026-10-22, after seeing the animated landing page and the coloured code blocks served
locally: "Oh, I love you. That's it. Do it. Do this, please. Do it."

Earlier request it answers, verbatim: "the entry page to have some, you know, more unique, fancy,
maybe even animated with waveforms and, you know, wires and patches and stuff flowing around, like
signals and stuff, and maybe a bit more, you know, color. You don't have to follow this greenish
background thing".

## Decisions

- D1 The landing page is a site-local override (`website/src/theme/ProductLandingPage.tsx`) of the
  docs-system product landing page; docs pages keep the docs-system style.
- D2 The hero animation is SVG and React with no added npm dependency; it shows a still frame under
  `prefers-reduced-motion` and pauses off-screen.
- D3 Code blocks load the docs-system `PRISM_ADDITIONAL_LANGUAGES` and site-local Prism themes in
  the landing palette; shell fences in hand-written pages are tagged `bash`.
- D4 The `features` and `related` sections of `product.json` are not shown on the landing page.
