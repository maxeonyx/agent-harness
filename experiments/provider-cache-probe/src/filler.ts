// Deterministic filler text, so a prefix is byte-identical between the two
// requests of a measurement but differs between measurements and between runs.
//
// Differing between measurements matters: every measurement must start from a
// cold cache, or measurement 7 reads the prefix measurement 3 happened to write
// and the result means nothing. The seed carries the run id and the measurement
// id, so nothing is shared unless a measurement deliberately shares a seed.

const WORDS = [
  "session",
  "context",
  "prefix",
  "append",
  "notice",
  "limb",
  "brain",
  "face",
  "harness",
  "record",
  "project",
  "compact",
  "refurbish",
  "initialise",
  "turn",
  "tool",
  "schema",
  "option",
  "provider",
  "request",
  "response",
  "budget",
  "cancel",
  "drain",
  "finalize",
  "outcome",
  "event",
  "stream",
  "snapshot",
  "rollup",
  "ledger",
  "journal",
  "durable",
  "transient",
  "warm",
  "cold",
  "expiry",
  "threshold",
  "policy",
  "economics",
  "measure",
  "evidence",
  "invariant",
  "gate",
  "review",
  "design",
  "experiment",
  "brief",
  "plan",
  "delegate",
  "subagent",
  "hierarchy",
  "scope",
  "sibling",
  "parent",
  "child",
  "topology",
  "deployment",
  "boundary",
  "identity",
  "provenance",
  "lifecycle",
];

/** mulberry32 over a FNV-1a hash of the seed. Small, deterministic, adequate. */
function rng(seed: string): () => number {
  let hash = 2166136261;
  for (let i = 0; i < seed.length; i++) {
    hash ^= seed.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  let state = hash >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * `words` random words in sentences of ten, headed by the seed so the text is
 * unmistakably this measurement's. Common English words run at roughly 1.3
 * tokens per word on both providers' tokenizers, so 1600 words lands near 2000
 * tokens — but the run reports the observed token counts and the report flags a
 * prefix that came in under the model's minimum cacheable length, rather than
 * trusting that ratio.
 */
export function filler(seed: string, words: number): string {
  const next = rng(seed);
  const out: string[] = [`Cache probe filler for ${seed}. Ignore the content.`];
  let sentence: string[] = [];
  for (let i = 0; i < words; i++) {
    sentence.push(WORDS[Math.floor(next() * WORDS.length)]);
    if (sentence.length === 10) {
      out.push(sentence.join(" ") + ".");
      sentence = [];
    }
  }
  if (sentence.length > 0) out.push(sentence.join(" ") + ".");
  return out.join("\n");
}

/** Words used for an ordinary prefix: comfortably over a 1,024-token minimum. */
export const STANDARD_WORDS = 1600;

/**
 * Words used where the prefix must also be cacheable by the smaller model.
 * Claude Haiku 4.5's minimum cacheable prefix is 4,096 tokens against Sonnet
 * 5's 1,024, so a 2,000-token prefix would make the cross-model measurement
 * meaningless: the read could fail because Haiku cannot cache a prefix that
 * short, not because the cache key includes the model.
 */
export const LARGE_WORDS = 4200;

/** A prefix deliberately below every model's minimum cacheable length. */
export const TINY_WORDS = 120;
