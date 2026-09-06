# React Component Architecture - Create Reference

Use this file when designing a new component from scratch.

## Design Mindset

Start from intention, then decide structure:

1. What is this component for?
2. What should it own?
3. What should it not own?
4. What is public, and what stays private?

The goal is to arrive at a component that is small enough to explain, stable enough to reuse, and explicit enough to compose safely.

## Component Design Steps

### 1. Define Responsibility

Write a single-sentence reason for the component to exist. If that sentence contains multiple jobs, split the job or split the component.

### 2. Classify Component Type

Choose the smallest type that fits:

- Shared: reusable primitives with no feature rules.
- Feature: feature-specific UI and flow.
- Layout: page composition and screen structure.

### 3. Identify Data Sources

List the data inputs before adding state:

- Props
- Query or server data
- Context
- Global store
- URL/search params
- Form state
- Local state
- Ref

Ask who owns the data, who updates it, and who needs it to render.

### 4. Assign State Ownership

Use local state only when the component owns the behavior. Lift state only when another component needs it to render correctly.

Do not let a large API response force parent ownership automatically. Decide ownership from UI ownership, behavior ownership, and rendering dependency.

Ask whether child sections update independently, have different loading/error/refresh/empty states, need different cache lifecycles, or would re-render unrelated UI if coupled together.

### 5. Choose State vs Ref

Use state for UI-visible changes. Use refs for non-reactive values or imperative control.

If the answer is unclear, treat it as an API boundary decision and propose options before moving state:

1. Keep one large API.
2. Split into smaller APIs.
3. Keep one API but split client-side ownership by section hooks/selectors.

### 6. Define Component Contract

Expose task-oriented props and events. Prefer intent-based APIs over setter leaks.

### 7. Define Hook Boundary

Move state transitions, effects, async work, transformation, and orchestration into hooks when they clutter render.

### 8. Split UI If Necessary

Use container/view when orchestration and rendering start fighting each other.

### 9. Validate Architecture

Check responsibility, ownership, contract clarity, and render simplicity before finishing.

## Shared UI Rules

- Accept props only.
- Do not fetch data.
- Do not read global state.
- Do not contain feature-specific business logic.

## Feature UI Rules

- Keep feature logic in hooks.
- Keep render bodies thin.
- Prefer task-oriented events over private setters.

## Layout Rules

- Own structure, section placement, and responsive composition.
- Avoid deep domain behavior unless it changes the layout itself.

## Common Failure Modes

- Reaching for reuse before the API is stable.
- Creating local state for data that already has an owner.
- Mixing orchestration and rendering in the same component body.
- Exposing internal setters instead of meaningful events.

## Example Contract

```tsx
<ProductForm onSubmit={handleSubmit} />
```

## Example Hook Return

```tsx
const [state, actions] = useProductList();
```
