---
name: react-component-architecture
description: Reviews and designs React component architecture with clear responsibilities, state ownership, hook boundaries, and composition patterns. Use when designing or refactoring React components, splitting shared/feature/layout components, deciding state vs ref, extracting hooks, or reviewing complex React UI structure.
---

# React Component Architecture Guide

Use this skill to reason like a React architect before writing or changing component code.
## Architecture Principles
Core principle: `UI = f(state)`.
Design targets: maintainability, readability, useful reuse, clear ownership, and SOLID alignment without ceremony.
React philosophy: composition over inheritance, encapsulation through focused components/hooks, abstraction through simple APIs, and explicit contracts over shared mutable internals.
## Intent-First Workflows
### Create Workflow
Use this flow when designing a new component:
1. Define responsibility: what is this thing and why does it exist?
2. Classify component type: shared, feature/section, or layout/composition.
3. Identify data sources: props, query, context, store, URL, form, or ref.
4. Assign state ownership: who owns, modifies, and reads each value?
5. Choose state vs ref: does the value affect rendering?
6. Define component contract: what props/events are public?
7. Define hook boundary: what behavior leaves render?
8. Split UI if necessary: container, view, child sections, shared primitives.
9. Validate architecture: responsibility, ownership, contracts, render simplicity.

See [REFERENCE_CREATE.md](REFERENCE_CREATE.md) for examples and deeper design rules.
### Refactor Workflow
Use this flow when reviewing an existing component:
1. Identify responsibilities in the current code.
2. Identify data and state owners.
3. Find behavior that should move into hooks.
4. Extract child components by responsibility.
5. Simplify the render body.
6. Review the public API for intent-based props and events.
7. Validate ownership, contracts, and behavior preservation.

See [REFERENCE_REFACTOR.md](REFERENCE_REFACTOR.md) for extraction order, regression checks, and review questions.
## Component Types
Shared component: feature-agnostic, reusable, props-only; no fetching, global reads, or business rules.

Feature component: owns UI for one feature or flow; may coordinate feature context/data, but feature logic should move to hooks.

Layout component: owns screen structure, section placement, and responsive composition; avoid deep domain logic unless it directly shapes composition.

## Data Ownership

Identify data sources before creating state: props, server query, context, global state, URL/search params, form state, local state, or ref.

For each value, ask who owns it, who can modify it, and who needs it to render.

Rules:
- Avoid local state when another owner is authoritative.
- Lift state only when another component needs it to render correctly.
- Keep state local when one component owns the behavior.

API boundary rule: do not put state in a parent only because it fetches one large API response for many sections. API shape does not decide ownership by itself; UI ownership, behavior ownership, and rendering dependency do.

If the answer is unclear during refactor, treat it as an API question first and propose options before changing state ownership.

## State vs Ref

Use state when a value changes what the user sees: `isOpen`, `isLoading`, `searchText`, `selectedItem`.
Use ref when a value affects behavior but should not trigger render: `timerId`, `requestId`, `scrollPosition`, `formInstance`.
Use imperative refs sparingly for APIs like `focus`, `reset`, or `submit`; expose only the smallest useful surface with `useImperativeHandle`.

## Hook Architecture

Hooks own behavior behind the UI: state transitions, effects, async work, fetching/subscriptions, data transformation, event handler orchestration, and UI-specific business flow.

Prefer a return shape that separates state from actions:

```tsx
const [state, actions] = useProductList();
```

## Component Composition

Think in this order: hook -> container -> view -> shared UI.

Use container/view when orchestration and rendering are mixed. Containers call hooks and connect data; views render props and emit task-oriented events.

Keep render bodies thin: hook calls, conditional rendering, and JSX composition. Move calculations, side effects, async logic, and nested orchestration out of render.

## Decision Matrix

- Use state when the value changes UI.
- Use ref when the value is non-reactive or imperative.
- Lift state when another component needs it to render.
- Do not lift state only because data comes from one API.
- Split a component when it has multiple reasons to change.
- Create a hook when behavior clutters render or needs an explicit lifecycle.
- Create a shared component only when reuse is real and feature rules are absent.

## Validation Checklist

- [ ] Single responsibility, component type, and data ownership are clear.
- [ ] Logic lives in hooks/helpers, not render.
- [ ] Render body is thin.
- [ ] Public contracts expose intent, not internals.
- [ ] Shared UI is reusable and feature-agnostic.
