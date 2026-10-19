// Loads the real ember + forge, compiled to WebAssembly (crates/blaze-wasm), and calls them over a tiny C ABI.
type Exports = {
  memory: WebAssembly.Memory
  alloc(n: number): number
  dealloc(p: number, n: number): void
  lint(p: number, n: number): void
  format(p: number, n: number): void
  fix(p: number, n: number, unsafeFixes: number): void
  rules(): void
  result_ptr(): number
  result_len(): number
}

let cached: Promise<Exports> | null = null
export const wasmReady = () =>
  (cached ??= fetch(`${import.meta.env.BASE_URL}blaze.wasm`)
    .then((r) => r.arrayBuffer())
    .then((b) => WebAssembly.instantiate(b, {}))
    .then((r) => r.instance.exports as unknown as Exports))

export interface Diag { code: string; message: string; line: number; column: number; fix: boolean; safe: boolean }
export interface RuleInfo { code: string; name: string; summary: string; default: boolean; fixable: boolean; unsafe: boolean }
export type FmtResult = { ok: true; output: string } | { ok: false; error: string }

function call<T>(ex: Exports, fn: 'lint' | 'format' | 'fix', src: string, flag = 0): T {
  const bytes = new TextEncoder().encode(src)
  const ptr = ex.alloc(bytes.length)
  new Uint8Array(ex.memory.buffer, ptr, bytes.length).set(bytes)
  if (fn === 'fix') ex.fix(ptr, bytes.length, flag)
  else ex[fn](ptr, bytes.length)
  const out = new TextDecoder().decode(new Uint8Array(ex.memory.buffer, ex.result_ptr(), ex.result_len()))
  ex.dealloc(ptr, bytes.length)
  return JSON.parse(out) as T
}

export async function lint(src: string): Promise<{ diags: Diag[]; ms: number }> {
  const ex = await wasmReady()
  const t = performance.now()
  const diags = call<Diag[]>(ex, 'lint', src)
  return { diags, ms: performance.now() - t }
}

export async function format(src: string): Promise<{ res: FmtResult; ms: number }> {
  let ex = await wasmReady()
  const t = performance.now()
  try {
    return { res: call<FmtResult>(ex, 'format', src), ms: performance.now() - t }
  } catch {
    // a panic inside the printer traps the instance; start a fresh one
    cached = null
    ex = await wasmReady()
    return { res: { ok: false, error: 'unsupported syntax (printer limitation)' }, ms: performance.now() - t }
  }
}

export async function fixAll(src: string, unsafeFixes = false): Promise<{ output: string; applied: number }> {
  const ex = await wasmReady()
  return call(ex, 'fix', src, unsafeFixes ? 1 : 0)
}

export async function listRules(): Promise<RuleInfo[]> {
  const ex = await wasmReady()
  ex.rules()
  const out = new TextDecoder().decode(new Uint8Array(ex.memory.buffer, ex.result_ptr(), ex.result_len()))
  return JSON.parse(out) as RuleInfo[]
}
