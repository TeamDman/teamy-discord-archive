---
name: local-library-references
description: 'Use when working with library APIs and you need to inspect source, understand behavior, or learn usage patterns. Prefer local source repositories and workspace search over online documentation for serenity, discord-api-docs, figue, and facet.'
argument-hint: 'Which library or API are you investigating?'
---

# Local Library References First

Use this skill when you need to understand a library API, inspect implementation details, verify behavior, or find examples while working in this repository.

The default rule is simple: prefer local source references over online documentation.

When the code is not in one of the preferred local repositories, search the local cargo cache before going to the web.

## Primary Repositories

Check these local repositories first:

- `G:\Programming\Repos\serenity`
- `G:\Programming\Repos\discord-api-docs`
- `G:\Programming\Repos\figue`
- `G:\Programming\Repos\facet`

## When To Use

Use this skill when:

- You need to understand a library type, method, trait, or module.
- You want implementation details that public docs often omit.
- You need concrete examples of how an API is used.
- You want to verify behavior against real source instead of summaries.
- You are tempted to fetch online docs for a library that already exists locally.

## Lookup Order

Follow this order unless the task explicitly requires external documentation:

1. Search the local repository source code.
2. Search examples, tests, and docs inside the local repository.
3. Search the current workspace if the relevant repo is already attached.
4. If the repo exists locally but is not in the workspace, add it with `code -a <path>` so workspace tools can search it.
5. Search the local cargo cache.
6. Use online documentation only if the needed information is not available locally, or if the task specifically requires official published docs.

## Cargo Cache Search

When searching the cargo cache, use `teamy-mft query` against `$env:CARGO_HOME`.

Example:

```powershell
teamy-mft query --in $env:CARGO_HOME --limit 25 "facet Cargo.toml$"
```

This is useful for locating checked out git dependencies and crate sources in the local cargo cache before reaching for online docs.

## Procedure

1. Identify the library and the exact API surface you need to understand.
2. Decide which local repository is most likely authoritative.
3. Search for the symbol definition first.
4. Search for call sites, examples, tests, and docs in that repository.
5. If the library is not available in the preferred local repositories, search the cargo cache with `teamy-mft query`.
6. Read the implementation when behavior is unclear from signatures alone.
7. Compare examples and implementation details before answering or changing code.
8. Only fall back to online docs after local sources have been checked and found insufficient.

## Decision Rules

### Prefer local source immediately when:

- The question is about behavior, edge cases, defaults, or implementation details.
- The API is from one of the local repositories listed above.
- You need examples grounded in the actual version available on disk.
- There may be drift between published docs and the local checkout.

### Online docs are acceptable when:

- The local repository does not contain the relevant API.
- The cargo cache search does not turn up the relevant crate or source.
- The user explicitly asks for official online documentation.
- The task depends on hosted reference material not present in the repository.
- You need broader ecosystem documentation for a dependency that is not available locally.

## What To Inspect Locally

Prioritize these sources in order:

1. Type, function, trait, and module definitions.
2. Tests covering the API.
3. Examples and sample applications.
4. Repository docs such as README files and internal design notes.
5. Cargo cache checkouts and crate sources.
6. Call sites showing real usage patterns.

## Quality Checks

Before concluding you understand an API, verify that you have:

- Located the authoritative definition.
- Confirmed behavior from implementation or tests when needed.
- Checked at least one real usage example if the API is non-trivial.
- Avoided relying on online summaries when a local source of truth exists.
- Noted when your answer depends on local checkout state rather than published docs.

## Notes For This Repository

This workflow is especially useful in teamy-discord-archive when investigating Discord-related APIs. The local `serenity` and `discord-api-docs` repositories should usually be treated as the first stop.