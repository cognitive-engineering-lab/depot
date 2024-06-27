# Depot: A Javascript devtool orchestrator

<img width="1233" alt="Screen Shot 2023-07-18 at 11 26 06 AM" src="https://github.com/cognitive-engineering-lab/depot/assets/663326/49cb46f9-bc88-46f5-9a1c-71ee6f1ffdea">

Depot (formerly Graco) is a tool for orchestrating other Javascript devtools. As an analogy:
* Depot is like [Cargo], but for Javascript.
* Depot is like [create-react-app], but for people who like software engineering.
* Depot is like the [`"scripts"` field of package.json](https://docs.npmjs.com/cli/v9/using-npm/scripts), but with more power and flexibility.

Depot works on Javascript workspaces that have been created by Depot, specifically those using the [model JS workspace] format. Depot supports the following commands:

* `depot new` - creates a new workspace or package with devtools preinstalled
* `depot init` - installs workspace dependencies with [pnpm]
* `depot build` - type-checks with [Typescript], and:
  * For libraries, transpiles with [Typescript]
  * For scripts and websites, bundles with [Vite]
* `depot test` - runs tests with [Vitest]
* `depot fmt` - formats source files with [Biome]
* `depot doc` - generates documentation with [Typedoc]

A few benefits of using Depot:
* Depot works with either browser or Node packages.
* Depot automatically runs command dependencies. For example, `depot test` will run `depot build`, and `depot build` will run `depot init`.
* Depot provides an interactive terminal interface for showing the running output of processes when building in watch mode.


## Installation

The [install script](https://github.com/cognitive-engineering-lab/depot/blob/main/scripts/install.sh) will download a prebuilt binary if possible. Run the script as follows:

```
curl https://raw.githubusercontent.com/cognitive-engineering-lab/depot/main/scripts/install.sh | sh
```

Alternatively, you can follow one of these installation methods:

### From crates.io

```
cargo install depot-js --locked
```

### From source

```
git clone https://github.com/cognitive-engineering-lab/depot
cd depot
cargo install --path crates/depot --locked
```

## Usage

To get started, create a new package:

```
depot new my-lib
```

You can specify `--target <lib|site|script>` to indicate that the package is a library (a Javascript package used by other packages), a website (an HTML site that uses Javascript), or a script (a Javascript program that would be either run on the CLI or included as a `<script>` tag.) You can also specify `--platform <browser|node>` depending on whether your package is intended to run in the browser or via NodeJS.

You can also create a workspace and a package within that workspace, like this:

```
depot new --workspace my-workspace
cd my-workspace
depot new my-lib
```

Inside the workspace, you can build all packages like this:

```
depot build
```

This generates a `<package>/dist` directory containing the built files. You can run in watch mode by passing `-w` like this:

```
depot build -w
```

Additional documentation about each command will be created soon once the Depot design is finalized.


## Projects using Depot

Depot is used in a few of our projects:
* [mdbook-quiz](https://github.com/cognitive-engineering-lab/mdbook-quiz/tree/main/js)
* [aquascope](https://github.com/cognitive-engineering-lab/aquascope/tree/main/frontend)

[model JS workspace]: https://github.com/willcrichton/model-js-workspace/
[Cargo]: https://doc.rust-lang.org/cargo/
[create-react-app]: https://create-react-app.dev/
[Typescript]: https://www.typescriptlang.org/
[Vite]: https://vitejs.dev/
[Vitest]: https://vitest.dev/
[Biome]: https://biomejs.dev/
[Typedoc]: https://typedoc.org/
[pnpm]: https://pnpm.io/
