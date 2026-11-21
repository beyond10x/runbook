// Does context.setOffline change navigator.onLine and fire the window 'offline'/'online' events?
import {chromium} from 'playwright';
import http from 'node:http';
const srv = http.createServer((q, r) => { r.setHeader('content-type', 'text/html'); r.end('<h1>x</h1>'); }).listen(0, '127.0.0.1');
await new Promise(r => srv.on('listening', r));
const port = (srv.address() as any).port;
const b = await chromium.launch();
const c = await b.newContext();
const p = await c.newPage();
await p.goto(`http://127.0.0.1:${port}/`);
await p.evaluate(() => { (window as any).__ev = []; addEventListener('online', () => (window as any).__ev.push('online')); addEventListener('offline', () => (window as any).__ev.push('offline')); });
console.log('onLine before', await p.evaluate(() => navigator.onLine));
await c.setOffline(true); await p.waitForTimeout(1000);
console.log('onLine offline', await p.evaluate(() => navigator.onLine), await p.evaluate(() => (window as any).__ev));
await c.setOffline(false); await p.waitForTimeout(1000);
console.log('onLine back', await p.evaluate(() => navigator.onLine), await p.evaluate(() => (window as any).__ev));
await b.close(); srv.close();
