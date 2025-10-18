Before changes run
- bun install

After changes run:
- bun run check
- bun run lint-fix and after you should add possible lint changes to the commit.
- bun run test
- bun run format-fix and after you should add the possible format changes to the commit.


Dont update readme unless specifically instructed

When making changes always think about unit tests you could implement but always prioritize finding relevant unit tests and updating them first.