---
status: accepted
---

# Resolve PATH from the login shell / registry, not the inherited process env

A process launched from a desktop icon (not a terminal) does not reliably inherit the user's
full shell PATH — tools installed via nvm/asdf-style version managers can be invisible to it,
so naively spawning `claude` via the process's inherited `PATH` would fail unpredictably
depending on how argus was launched. `ResolveStartupPathUseCase` resolves PATH once per app
process and caches it, via platform-specific adapters: on Unix, running the user's login
shell (`$SHELL -lic 'echo -n $PATH'`); on Windows, reading and concatenating
`HKCU\Environment` and `HKLM\SYSTEM\...\Session Manager\Environment` directly from the
registry rather than trusting `std::env::var("PATH")`. The resolved value is injected
explicitly as `SpawnSpec.env_path` into every `claude` spawn, overriding whatever PATH the
host process itself had.

This means every workspace spawn depends on a resolution step that can fail (slow/misconfigured
login shell, unreadable registry keys) — `create_workspace_*` commands surface that failure to
the user rather than silently falling back to a broken PATH, since a broken PATH would just
fail to find `claude` anyway with a much more confusing error.
