# Adopting Bap in an existing Node project

Bap is an incremental runtime option: `package.json`, lockfiles, CI tooling, and
editor setup stay untouched while you evaluate it per service.

## The pathway

1. **Run the existing test suite under Bap.** Point your runner at `bap run` for
   each test file (or a thin wrapper script). Most failures at this stage are
   missing Node builtins.
2. **Diagnose with `bap doctor`.** Run it in the project root; it scans source
   files for imports and requires, then classifies every unique specifier:

   - `supported` — served by Bap's `node:` shim today;
   - `NOT IMPLEMENTED` — a Node builtin Bap knows about but does not ship yet;
   - `missing` — a bare npm package; map it locally or wait for the package
     manager milestone;
   - `local` — relative paths and configured aliases, checked on disk when run.

   Add `--json` for machine-readable output.
3. **Close gaps with aliases.** For small dependencies, vendor the module and
   map it in `bap.json`; larger gaps become upstream feature requests.
4. **Flip the runtime config per service.** Each service that passes its suite
   declares `"runtime": "bap"` in its own `bap.json`. Nothing else changes:
   same scripts, same CI, same container entrypoints.
5. **Keep the escape hatch.** Removing `bap.json` returns the service to
   Node/Bun with zero code changes.

## bap.json

```json
{
  "runtime": "bap",
  "moduleFormat": "esm",
  "aliases": {
    "left-pad": "./vendor/left-pad.js"
  }
}
```

- `runtime`: only `"bap"` is accepted; anything else fails fast so typos do not
  silently run under another runtime's config.
- `moduleFormat`: pins `.js` classification to `esm` or `cjs` project-wide;
  omit it to let Bap decide per file (extension, markers, nearest
  `package.json`).
- `aliases`: bare specifier to local file, resolved from the directory holding
  `bap.json`. Works identically for `import` and `require`.

## What compatibility means here

Bap targets the core surface real services use: ESM first, a CommonJS loader
with best-effort interop, `node:path`, `node:fs`, `node:util`, `node:events`,
`node:buffer`, and `node:process`. Unsupported modules fail loudly at load time
with a clear message instead of misbehaving later. Measured compatibility over
a pinned corpus ships as `bap report compat`; the number is reported honestly,
never assumed.
