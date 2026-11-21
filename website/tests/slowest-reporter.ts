import type {FullResult, Reporter, TestCase, TestResult} from '@playwright/test/reporter';

export default class SlowestReporter implements Reporter {
  private times: Array<{title: string; ms: number}> = [];
  private start = Date.now();

  onTestEnd(test: TestCase, result: TestResult): void {
    this.times.push({title: `${test.parent.project()?.name ?? ''} › ${test.titlePath().slice(2).join(' › ')}`, ms: result.duration});
  }

  onEnd(result: FullResult): void {
    const slowest = [...this.times].sort((a, b) => b.ms - a.ms).slice(0, 10);
    console.log(`\nTen slowest tests (${this.times.length} tests, ${((Date.now() - this.start) / 1000).toFixed(1)} s wall, ${result.status}):`);
    for (const t of slowest) console.log(`  ${(t.ms / 1000).toFixed(1).padStart(6)} s  ${t.title}`);
  }
}
