# Depot: A Javascript devtool orchestrator

Depot (formerly Graco) is a tool for orchestrating other Javascript devtools. As an analogy:
* Depot is like [Cargo], but for Javascript.
* Depot is like [create-react-app], but for people who like software engineering.
* Depot is like the `"scripts'` field of package.json, but with more power and flexibility.

Depot works on Javascript workspaces that have been created by Depot, specifically those that match the [model JS workspace]. Depot supports the following commands:

* `depot new` - creates a new workspace or package with devtools preinstalled
* `depot init` - installs workspace dependencies with [pnpm]
* `depot build` - type-checks with [Typescript], and:
  * For libraries, transpiles with [Typescript]
  * For scripts and websites, bundles with [Vite]
* `depot test` - runs tests with [Vitest]
* `depot fmt` - formats source files with [Prettier]
* `depot doc` - generates documentation with [Typedoc]

A few benefits of using Depot:
* Depot works with either browser or Node packages.
* Depot automatically runs command dependencies. For example, `depot test` will run `depot build`, and `depot build` will run `depot init`.
* Depot provides an interactive terminal interface for showing the running output of processes when building in watch mode.

## Installation

Run the following script to install the `depot` binary:

```
curl https://raw.githubusercontent.com/cognitive-engineering-lab/depot/main/scripts/install.sh | sh
```

## Usage

Additional documentation coming soon!


[model JS workspace]: https://github.com/willcrichton/model-js-workspace/
[Cargo]: https://doc.rust-lang.org/cargo/
[create-react-app]: https://create-react-app.dev/
[Typescript]: https://www.typescriptlang.org/
[Vite]: https://vitejs.dev/
[Vitest]: https://vitest.dev/
[Prettier]: https://prettier.io/
[Typedoc]: https://typedoc.org/
[pnpm]: https://pnpm.io/
