// How each blinded product is started, taken from its README. The harness runs in a network
// namespace with loopback only; each process runs in the confinement of confine.ts.
import {spawn, ChildProcess} from 'node:child_process';
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import {bwrapArgs} from './confine.ts';

export const Q = process.env.QUALITY_DIR ?? path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');

export async function freePort(): Promise<number> {
  return new Promise((res, rej) => {
    const s = net.createServer();
    s.listen(0, '127.0.0.1', () => { const p = (s.address() as net.AddressInfo).port; s.close(() => res(p)); });
    s.on('error', rej);
  });
}

export interface Start { readme: string; procs: {bin: string; args: string[]}[]; }
// `{port}`, `{port2}` are free ports; `{data}` is the product's data directory.
export const START: Record<string, Start> = {
  p1: {readme: 'cargo run --release -- <port> <data-file> [bind-address]',
    procs: [{bin: 'target/release/todo-server', args: ['{port}', '{data}/todo.dat', '127.0.0.1']}]},
  p2: {readme: 'cargo run --locked --bin sync_service -- <sync-port> <sync-data-file>; cargo run --locked --bin todo-app -- <port> <data-file> <sync-host:port> <partner-id> <partner-id>',
    procs: [{bin: 'target/debug/sync_service', args: ['{port2}', '{data}/sync.dat']},
      {bin: 'target/debug/todo-app', args: ['{port}', '{data}/app.dat', '127.0.0.1:{port2}',
        '11111111-1111-4111-8111-111111111111', '22222222-2222-4222-8222-222222222222']}]},
  p3: {readme: 'cargo build; ./target/debug/todod <port> <data-file>',
    procs: [{bin: 'target/debug/todod', args: ['{port}', '{data}/tasks.txt']}]},
  p4: {readme: 'cargo run --locked -- <port> <data-file> [secret]',
    procs: [{bin: 'target/debug/todo-service', args: ['{port}', '{data}/todo.dat']}]},
  p5: {readme: 'cargo build --release; target/release/todo-daemon <port> <data-file>',
    procs: [{bin: 'target/release/todo-daemon', args: ['{port}', '{data}/a.tsv']}]},
  p6: {readme: 'cargo run --locked -- <port> <data-file>',
    procs: [{bin: 'target/debug/todo-service', args: ['{port}', '{data}/list.dat']}]},
  p7: {readme: 'cargo run --release -- <port> <data-file>',
    procs: [{bin: 'target/release/todo', args: ['{port}', '{data}/tasks.txt']}]},
  p8: {readme: 'cargo run --release --bin list-service -- <port> <data-file>',
    procs: [{bin: 'target/release/list-service', args: ['{port}', '{data}/list.dat']}]},
  p9: {readme: 'cargo run --locked --bin todo-service -- <port> <data-file>',
    procs: [{bin: 'target/debug/todo-service', args: ['{port}', '{data}/todo.dat']}]},
};

export class Running {
  procs: ChildProcess[] = [];
  out = '';
  id: string; port: number; port2: number;
  constructor(id: string, port: number, port2: number) { this.id = id; this.port = port; this.port2 = port2; }
  get base() { return `http://127.0.0.1:${this.port}`; }
  dir() { return path.join(Q, 'products', this.id); }
  async start(): Promise<{ok: boolean; status?: number; ms: number}> {
    const dir = this.dir(), data = path.join(dir, 'data');
    fs.mkdirSync(data, {recursive: true});
    const ch = path.join(Q, 'cargo-home', this.id);
    const t0 = Date.now();
    for (const p of START[this.id].procs) {
      const sub = (s: string) => s.replace('{port2}', String(this.port2)).replace('{port}', String(this.port)).replace('{data}', data);
      const a = bwrapArgs('serve', dir, ch, [path.join(dir, p.bin), ...p.args.map(sub)]);
      const c = spawn('/usr/bin/bwrap', a, {env: {}, stdio: ['ignore', 'pipe', 'pipe']});
      c.stdout!.on('data', d => { this.out += d; });
      c.stderr!.on('data', d => { this.out += d; });
      this.procs.push(c);
      await new Promise(r => setTimeout(r, 300));
    }
    while (Date.now() - t0 < 15000) {
      try {
        const r = await fetch(this.base + '/', {signal: AbortSignal.timeout(2000)});
        await r.arrayBuffer();
        return {ok: true, status: r.status, ms: Date.now() - t0};
      } catch { await new Promise(r => setTimeout(r, 250)); }
    }
    return {ok: false, ms: Date.now() - t0};
  }
  async stop() {
    for (const c of this.procs) {
      if (c.exitCode === null && c.signalCode === null) {
        c.kill('SIGKILL');
        await new Promise(r => c.once('exit', r));
      }
    }
    this.procs = [];
    // wait until the port is closed
    for (let i = 0; i < 40; i++) {
      try { await fetch(this.base + '/', {signal: AbortSignal.timeout(300)}); await new Promise(r => setTimeout(r, 100)); }
      catch { return; }
    }
  }
}

export function resetData(id: string) {
  fs.rmSync(path.join(Q, 'products', id, 'data'), {recursive: true, force: true});
}
