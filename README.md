# Graco: a JS devtool orchestrator

![Tests](https://github.com/willcrichton/graco/actions/workflows/tests.yaml/badge.svg)
![npm](https://img.shields.io/npm/v/graco)

<img width="700" alt="Screen Shot 2022-10-31 at 11 04 35 PM" src="https://user-images.githubusercontent.com/663326/199170700-60027b7f-dfaa-43d6-8afe-3296d8307727.png">

I'm so goddamn tired of dealing with the Javascript devtool ecosystem. But nonetheless, I must make websites. Therefore I combined my frustration and hubris into Graco &mdash; it's like Cargo, for Javascript. Graco is a single script that orchestrates many JS devtools. You can do:

* `graco new` &mdash; create a new skeleton library, binary, or website
* `graco build` &mdash; checks via [Typescript](https://www.typescriptlang.org/), builds via [Vite](https://vitejs.dev/) for websites and [esbuild](https://esbuild.github.io/) otherwise, and lints via [eslint](https://eslint.org/)
* `graco fmt` &mdash; formats via [Prettier](https://prettier.io/)
* `graco test` &mdash; tests via [Jest](https://jestjs.io/)

(In the future: `graco doc` will document via [typedoc](https://typedoc.org/), and `graco publish` will publish via [lerna](https://lerna.js.org/).)

All of these commands work on individual packages or monorepos with multiple packages. Graco provides a default configuration for every tool, which you can eject and customize if necessary.

## Usage

Don't use this right now. I'm working on it. But if you *really* want to try...

```
npm install -g graco
graco new --target bin --platform browser my-website
cd my-website
graco build -w
```

Then go to <http://localhost:8000>.
