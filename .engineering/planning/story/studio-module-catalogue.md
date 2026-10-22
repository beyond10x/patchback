---
format: aep.planning-md/3
id: story:studio-module-catalogue
kind: story
status: draft
title: Module catalogue view
relations:
- decomposes: epic:patch-studio
- informed_by: executable-system-specification:patchback-contracts
- depends_on: story:parameter-contracts
revision: 1
---
# Module catalogue view

## Outcome

The studio lists every atomic module kind of a topology with its ports (direction, rate, meaning)
and its parameter contracts (index, path, mapping, physical range, default), so an engineer can
read what a module accepts and what the fit may change on it. Source:
`patchback.studio.PublishCatalogue`, event `CataloguePublished`; a kind appears once and a port name
once per kind.

## DESIGN.md

§ 5.1, § 5.2, § 5.4.

## Acceptance

- spec/scenarios/studio-module-catalogue-published.yaml
- spec/scenarios/studio-module-catalogue-duplicate-kind.yaml
- spec/scenarios/studio-module-catalogue-duplicate-port.yaml

## Numeric obligations

None beyond the parameter contracts' own (held by parameter-contracts).

## Open questions

- Port lists per node kind are not in DESIGN.md; the fixture's ports are illustrative.
- A browser tool in TypeScript against § 3 and § 4.1: needs an ADR before the studio leaves
  prototype.
