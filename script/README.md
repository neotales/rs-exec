# neotales-script

Builds `neotales-exec` commands for script runtimes.

By [x0craft](https://github.com/x0craft).

[![Neotales](https://github.com/neotales/brand/raw/main/assets/logo.png)](https://github.com/neotales/rs-exec)

<!-- badges: crates.io, docs.rs, CI. Add once the first version exists. -->

## Install 📦

```sh
cargo add neotales-script
```

## What you can do 🧰

**Get the arguments the runtime expects.** Each module knows how its runtime wants a file or a
piece of source, so the caller never spells out `-c`, `--file`, or `run`. `bash::inline` and
`sh::inline` pass a `$0` placeholder before your arguments, so the first one reaches the script as
`$1`.

```rust
use neotales_script::{bash, node};

let command = bash::inline("echo hi", []);
assert_eq!(command.arguments()[4], "-c");

let command = node::file("app.js", ["--watch"]);
assert_eq!(command.arguments(), ["app.js", "--watch"]);
```

**Give it a string and let it choose.** `script` treats a single line that ends in a known extension
as a file and runs anything else inline. A line with a `\n` in it is always source.

```rust
use neotales_script::{bash, cmd, golang};

assert_eq!(bash::script("build.sh", []).arguments()[4], "build.sh");
assert_eq!(bash::script("echo hi", []).arguments()[5], "echo hi");
assert_eq!(cmd::script("setup.bat", []).arguments()[2], "setup.bat");
assert_eq!(golang::script("main.go", []).unwrap().arguments()[1], "main.go");
```

**Run the command like any other.** Every constructor returns an owned `neotales_exec::Command`,
so the working directory, the environment, and the logger are set on the same chain.

```rust,no_run
use neotales_script::bash;

let output = bash::inline("echo hi", [])
    .current_dir(".")
    .env("MODE", "test")
    .output()
    .unwrap();
assert_eq!(output.text().trim(), "hi");
```

**Pass source to Python, Ruby, Perl, or Deno.** Each one has its own inline flag, and Perl keeps a
`--` so a leading dash reaches `@ARGV` instead of perl itself.

```rust
use neotales_script::{perl, python};

let command = python::inline("print(1)", ["a"]);
assert_eq!(command.arguments(), ["-c", "print(1)", "a"]);

let command = perl::inline("print 1", ["-n"]);
assert_eq!(command.arguments(), ["-e", "print 1", "--", "-n"]);
```

**Run inline Go and .NET source.** `golang::inline` and `dotnet::inline` write the source to a
fresh private file in the user's directory, then run it. The command records the path, and
the file stays there.

On Windows, privacy relies on the per-user `LocalAppData` directory and reparse-point refusal, not a custom ACL.

```rust
use neotales_script::{dotnet, golang};

let go = golang::inline("package main\nfunc main() {}\n", []).unwrap();
assert!(go.temp_file().is_some());

let cs = dotnet::inline("System.Console.WriteLine(1);", []).unwrap();
assert!(cs.temp_file().is_some());
```

**Runtime modules.** `bash`, `sh`, `node`, `bun`, `deno`, `golang`, `dotnet`, `powershell`, `pwsh`,
`cmd`, `python`, `ruby`, `perl`, `uv`, `php`, `fish`, and `nushell`. Each one resolves its
executable through `neotales_exec::find`, so an environment variable such as `RUN_NODE_EXE`
overrides the search, and a missing runtime falls back to the bare program name. The `nushell`
module runs the `nu` executable.

## Licence

[MIT](../LICENSE.md)
