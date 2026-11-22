// Prints the ten slowest tests of a run and fails the run that took longer than its budget. The same
// shape as the docs site's reporter (website/tests/slowest-reporter.ts); the two packages share no code.
// The budget is the reporter's option (`['./test/slowest-reporter.mjs', {budgetMs: 120000}]`): a run over
// it ends failed, whatever its tests said. The runner's own limits are in playwright.config.mjs.
export default class SlowestReporter {
  constructor(options = {}) {
    this.budgetMs = options.budgetMs ?? Infinity;
    this.times = [];
    this.start = Date.now();
    this.expects = 0;
  }

  printsToStdio() { return false; }

  onStepEnd(_test, _result, step) {
    if (step.category === 'expect') this.expects++;
  }

  onTestEnd(test, result) {
    if (result.status === 'skipped') return;
    this.times.push({title: `${test.parent.project()?.name ?? ''} › ${test.titlePath().slice(2).join(' › ')}`, ms: result.duration});
  }

  onEnd(result) {
    const wall = Date.now() - this.start;
    const slowest = [...this.times].sort((a, b) => b.ms - a.ms).slice(0, 10);
    console.log(`\nTen slowest tests (${this.times.length} tests run, ${this.expects} expect calls, ${(wall / 1000).toFixed(1)} s wall, ${result.status}):`);
    for (const t of slowest) console.log(`  ${(t.ms / 1000).toFixed(1).padStart(6)} s  ${t.title}`);
    if (wall > this.budgetMs) {
      console.log(`\nThe run took ${(wall / 1000).toFixed(1)} s, over its budget of ${(this.budgetMs / 1000).toFixed(0)} s: a defect of the test step, not a reason to raise the budget.`);
      return {status: 'failed'};
    }
  }
}
