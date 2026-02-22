# Instructions for agents

## 1. After changes

**If changes made to typescript code run:**

- bun run check
- bun run lint-fix and after you should add possible lint changes to the commit.
- bun run test
- bun run format-fix and after you should add the possible format changes to the commit.

**If changes made to rust code run:**

- cargo build --workspace
- cargo test --workspace

## 2. Documentation

When making changes always check docs folder if it needs to be updated and update it if needed.

## 3. Testing

- When making changes always check if there are relevant unit tests and if not add them. If there are relevant unit tests, run them and update them if needed.
- When making changes to rendering take screenshots into ./workdir folder to check your work visually and make changes if needed.