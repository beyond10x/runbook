import type {FullResult, Reporter, TestCase, TestResult, TestStep} from '@playwright/test/reporter';

export default class SlowestReporter implements Reporter {
  private times: Array<{title: string; ms: number}> = [];
  private start = Date.now();
  private expects = 0;

  onStepEnd(_test: TestCase, _result: TestResult, step: TestStep): void {
    if (step.category === 'expect') this.expects++;
  }

  onTestEnd(test: TestCase, result: TestResult): void {
    this.times.push({title: `${test.parent.project()?.name ?? ''} › ${test.titlePath().slice(2).join(' › ')}`, ms: result.duration});
  }

  onEnd(result: FullResult): void {
    const slowest = [...this.times].sort((a, b) => b.ms - a.ms).slice(0, 10);
    console.log(`\nTen slowest tests (${this.times.length} tests, ${this.expects} expect calls, ${((Date.now() - this.start) / 1000).toFixed(1)} s wall, ${result.status}):`);
    for (const t of slowest) console.log(`  ${(t.ms / 1000).toFixed(1).padStart(6)} s  ${t.title}`);
  }
}
