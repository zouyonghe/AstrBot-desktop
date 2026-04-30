# Desktop Plugin Dependency Boundary Design

## Context

Packaged desktop builds run AstrBot inside a bundled CPython runtime. Runtime plugin
dependencies are installed into `ASTRBOT_ROOT/data/site-packages` and are then
prepended to `sys.path` by AstrBot's pip installer.

This allows plugin-only dependencies to work, but it also allows plugin installs to
shadow packages already used by the bundled backend runtime. Packages such as
`openai`, `pydantic`, `fastapi`, `numpy`, and PyObjC modules are unsafe to hot-reload
inside a running backend process. Installing Cua exposed this failure mode: pip
installed a second dependency set into plugin `site-packages`; the installer tried to
prefer those modules in-process, and the running OpenAI client later saw incompatible
class identities.

## Goals

- Keep packaged desktop plugin installs from replacing bundled core runtime modules.
- Preserve normal plugin dependency installation when dependencies are not core
  runtime packages.
- Make the desktop bundle self-describing enough that runtime code can distinguish
  bundled core dependencies from plugin dependencies.
- Favor explicit incompatibility or restart-required behavior over in-process module
  replacement.

## Non-Goals

- Full per-plugin virtual environment isolation.
- Rewriting AstrBot's plugin manager.
- Guaranteeing that every PyPI package can coexist in the backend process.

## Design

The desktop backend build writes a `runtime-core-lock.json` file into
`resources/backend/app`. The lock captures distributions installed in the bundled
runtime, their versions, and top-level import modules. The launcher exposes this file
to the backend through `ASTRBOT_DESKTOP_CORE_LOCK_PATH`.

When running in packaged desktop mode, AstrBot's pip installer reads the lock and uses
it for two protections:

1. Add constraints for locked distributions so plugin pip installs cannot upgrade or
   downgrade bundled runtime packages.
2. Avoid preferring locked top-level modules from `data/site-packages` after install.

If pip cannot satisfy a plugin because it truly requires a different core dependency
version, installation fails with the existing dependency conflict path. This is
intentional: a clear install-time incompatibility is safer than corrupting the live
backend process.

## Data Flow

1. `scripts/backend/build-backend.mjs` installs backend requirements into the copied
   runtime.
2. The build script runs a small Python helper against that runtime to enumerate
   installed distributions.
3. The helper writes `app/runtime-core-lock.json`.
4. `launch_backend.py` sets `ASTRBOT_DESKTOP_CORE_LOCK_PATH` before running
   `main.py`.
5. The packaged backend pip installer reads the lock while installing plugin
   requirements and while attempting dependency preference.

## Error Handling

- If the lock cannot be generated during resource preparation, the backend build
  fails. A packaged desktop runtime without this boundary is not considered safe.
- If the lock cannot be read at runtime, AstrBot falls back to existing behavior and
  logs a warning. This keeps old custom launch setups from breaking.
- If a plugin conflicts with locked core dependencies, the install fails with a
  dependency conflict message.

## Testing

- Script tests cover lock generation and launcher environment wiring.
- AstrBot pip installer tests cover adding lock constraints and skipping locked
  modules during post-install preference.
