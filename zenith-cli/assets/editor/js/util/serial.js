// A one-at-a-time runner: tasks run in call order, each after the one
// before it ended.

export class Serial {
  constructor() {
    this.tail = Promise.resolve();
    /** Tasks queued or running. */
    this.count = 0;
  }

  /**
   * Run `task` after every task queued before it. Resolves or rejects as
   * `task` does. A rejected task does not stop the tasks after it.
   */
  run(task) {
    this.count++;
    const step = this.tail.then(task);
    const done = () => {
      this.count--;
    };
    this.tail = step.then(done, done);
    return step;
  }

  /** `true` when no task is queued or running. */
  idle() {
    return this.count === 0;
  }
}
