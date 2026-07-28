# Local agent instructions

## Learn and document with every change

- Explain the concept, its purpose, and the relevant Rust, Wayland, or Linux
  mechanism before implementing it.
- Prefer the smallest change that produces a useful, demonstrable result.
- Explain every changed file and every important function.
- Run the relevant formatting, tests, lints, and mdBook build before handing
  work back. Record the actual commands and results in the matching stage
  chapter.
- Code changes that affect behavior, architecture, configuration, protocols,
  dependencies, setup, testing, or the roadmap are incomplete until the
  README, relevant mdBook chapters, and `docs/SUMMARY.md` navigation agree
  with them.
- Add a stage chapter and link it from `docs/SUMMARY.md` when a distinct
  milestone is completed.
- Keep desktop behavior independent from optional mobile UI and describe
  hardware such as the FP5 as a reference target unless code is intentionally
  device-specific.
- Keep Patin usable without 0xin. Compositor-specific integration belongs
  behind a replaceable adapter.
- Run `git diff --check` before handing work back.
- Do not create commits unless the user explicitly asks. After each milestone,
  list the files to stage and give one short example commit-message line.

