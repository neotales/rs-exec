# rs-exec

Two crates, the second built on the first:

- `neotales-exec` runs external commands and pipelines with owned, chainable commands and no
  shell, and finds the executable a caller needs to name.
- `neotales-script` builds `neotales-exec` commands for the argument shapes bash, sh, node, bun,
  Go, .NET, PowerShell and cmd expect.

`neotales-script` depends on `neotales-exec`. The edge does not run the other way: the runner knows
nothing about a script runtime, and a caller that only runs processes never pulls one in.

By [x0craft](https://github.com/x0craft).

[![Neotales](https://github.com/neotales/brand/raw/main/assets/logo.png)](https://github.com/neotales/rs-exec)

| Crate | Directory | README |
| --- | --- | --- |
| `neotales-exec` | `exec/` | [exec/README.md](exec/README.md) |
| `neotales-script` | `script/` | [script/README.md](script/README.md) |

Both crates share one version and one tag, and `release.yml` publishes `neotales-exec` first
because `neotales-script` names it as a dependency.
