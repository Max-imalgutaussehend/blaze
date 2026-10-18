// Loads the real ember + forge, compiled to WebAssembly (crates/blaze-wasm), and calls them over a tiny C ABI.
type Exports = {
  memory: WebAssembly.Memory
  alloc(n: number): number
  dealloc(p: number, n: number): void
  lint(p: number, n: number): void
  format(p: number, n: number): void
  result_ptr(): number
  result_len(): number
}

let cached: Promise<Exports> | null = null
export const wasmReady = () =>
  (cached ??= fetch(`${import.meta.env.BASE_URL}blaze.wasm`)
    .then((r) => r.arrayBuffer())
    .then((b) => WebAssembly.instantiate(b, {}))
    .then((r) => r.instance.exports as unknown as Exports))

export interface Diag { code: string; message: string; line: number; column: number }
export type FmtResult = { ok: true; output: string } | { ok: false; error: string }

function call<T>(ex: Exports, fn: 'lint' | 'format', src: string): T {
  const bytes = new TextEncoder().encode(src)
  const ptr = ex.alloc(bytes.length)
  new Uint8Array(ex.memory.buffer, ptr, bytes.length).set(bytes)
  ex[fn](ptr, bytes.length)
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
