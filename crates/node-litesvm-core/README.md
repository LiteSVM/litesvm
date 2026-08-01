# LiteSVM Core

`litesvm-core` contains the raw Node.js NAPI bindings for
[LiteSVM](https://github.com/LiteSVM/litesvm). It is the shared native runtime
used by the SDK-specific `litesvm` and `litesvm-web3js` packages.

Most applications should install one of the SDK wrappers instead:

- `litesvm` for `@solana/kit`
- `litesvm-web3js` for `@solana/web3.js` v3

Install this package directly only when you need the byte-oriented generated
NAPI API:

```sh
yarn add litesvm-core
```

```ts
import { LiteSvm } from "litesvm-core";

const svm = new LiteSvm();
```

The core package intentionally has no dependency on either Solana JavaScript
SDK.
