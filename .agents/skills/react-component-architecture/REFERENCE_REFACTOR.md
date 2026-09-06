# React Component Architecture - Refactor Reference

Use this file when improving an existing component without changing behavior.

## Refactor Mindset

Refactor starts from the current shape of the code, not from the ideal shape.

The first job is to understand what must stay stable, then isolate where behavior, ownership, and composition can be clarified without changing what the user sees.

## Refactoring Workflow

### 1. Identify Responsibilities

Read the current component and write down every real job it is doing. If the list is long, the component is already doing too much.

### 2. Identify Data and State Owners

Map each value to its owner before moving code. This prevents accidental duplication of state or broken synchronization.

If one parent fetches a large API for multiple sections, do not assume parent state ownership is correct. Check whether the API boundary is the real problem before refactoring the component tree.

Ask whether sections update independently, fail independently, refresh independently, or need distinct loading/error/empty/cache lifecycles. If the answer is unclear, prepare a proposal before changing ownership.

### 3. Move Business Logic Into Hooks

Extract behavior first when it is tangled with render logic, side effects, async work, or state transitions.

### 4. Extract Child Components By Responsibility

Once logic is separated, split UI by ownership boundaries, not by arbitrary visual chunks.

### 5. Simplify Render Body

The render path should become a thin composition layer after the logic moves out.

### 6. Review Public API

Check whether the current props expose internals, setters, or incidental implementation detail. Replace them with intent-based events when possible.

If API shape is coupling unrelated sections together, compare:

1. Keep one large API.
2. Split into smaller APIs.
3. Keep one API but split client-side ownership by section hooks/selectors.

Recommended output:

- Current problem
- Affected sections
- Current API shape
- Proposed API options
- Pros and cons
- Migration cost
- Recommended direction

### 7. Validate Ownership And Behavior

Confirm that the behavior still matches the original component and that data still has one clear owner.

## Extraction Order

Prefer this order when the code is messy:

1. State ownership
2. Hook extraction
3. Container/view split
4. Child component extraction
5. API cleanup

Do not optimize for elegance before the behavior is isolated.

## Regression Checks

- [ ] Behavior should remain unchanged unless explicitly intended.
- [ ] Existing event flow should still work.
- [ ] Controlled inputs should still stay controlled.
- [ ] Derived values should still match the original output.
- [ ] Side effects should still run in the same lifecycle phase.

## Refactor Decision Rules

- Create a hook when behavior clutters render or has its own lifecycle.
- Split a component when it has multiple reasons to change.
- Keep props stable while refactoring if callers depend on them.
- Introduce new abstractions only after the current behavior is understood.

## Review Questions

- What is the minimum change that clarifies this component?
- Which logic should move first?
- What must not change?
- Which prop is exposing a private detail?
- Which state is really owned elsewhere?

## Common Refactor Outcomes

- A thin container with hooks.
- A focused view component.
- A smaller shared primitive.
- A clearer contract with fewer props.
- Less duplicated logic across siblings.
