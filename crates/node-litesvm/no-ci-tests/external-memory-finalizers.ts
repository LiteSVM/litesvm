// #345 verification, async shape: yields between instances (like a real server
// handling requests). Without --expose-gc.
//   node -r ts-node/register -r tsconfig-paths/register no-ci-tests/external-memory-finalizers.ts [N]
import { LiteSVM } from "index";

// ES2021 API not in the project's es2020 lib; declare the minimal surface.
declare const FinalizationRegistry: {
  new <T>(cleanup: (held: T) => void): {
    register(target: object, held: T): void;
  };
};

const MB = (n: number) => +(n / 1048576).toFixed(1);
const rss = () => process.memoryUsage.rss();
const N = Number(process.argv[2] || 400);
const YIELD_EVERY = Number(process.argv[3] || 5);

let collected = 0;
const reg = new FinalizationRegistry<number>(() => {
  collected++;
});

const build = (): LiteSVM =>
  new LiteSVM()
    .withBlockhashCheck(false)
    .withSigverify(false)
    .withDefaultPrograms()
    .withNativeMints()
    .withSysvars()
    .withBuiltins()
    .withPrecompiles();

const yieldTick = () => new Promise<void>((resolve) => setImmediate(resolve));

(async () => {
  const start = rss();
  let peak = start;
  console.log(`start rss=${MB(start)}MB N=${N} yieldEvery=${YIELD_EVERY} node=${process.version}`);

  const t0 = Date.now();
  for (let i = 1; i <= N; i++) {
    let s: LiteSVM | null = build();
    reg.register(s, i);
    s = null;
    if (i % YIELD_EVERY === 0) await yieldTick();
    if (i % 50 === 0) {
      const r = rss();
      if (r > peak) peak = r;
      console.log(
        `iter ${i}: rss=${MB(r)}MB peak=${MB(peak)} collected=${collected} heap=${MB(process.memoryUsage().heapUsed)}MB`,
      );
    }
  }
  console.log(
    `loop done in ${Date.now() - t0}ms rss=${MB(rss())} peak=${MB(peak)} collected=${collected}`,
  );

  let idle = 0;
  const idleInterval = setInterval(() => {
    console.log(`idle ${idle}: rss=${MB(rss())}MB collected=${collected}`);
    if (++idle >= 6) {
      clearInterval(idleInterval);
      const final = rss();
      console.log(JSON.stringify({ N, start: MB(start), peak: MB(peak), final: MB(final), collected }));
      const midRunCollection = collected > 0 && peak < start + N * 7.5 * 1048576 * 0.6;
      console.log(
        midRunCollection
          ? "PASS: GC interleaved with the workload, peak RSS far below unbounded growth"
          : "FAIL: GC only at idle, peak RSS near unbounded growth",
      );
      process.exit(0);
    }
  }, 500);
})();
