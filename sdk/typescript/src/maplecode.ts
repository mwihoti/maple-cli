import { MaplecodeOptions } from "./maplecodeOptions";
import { MaplecodeExec } from "./exec";
import { Thread } from "./thread";
import { ThreadOptions } from "./threadOptions";

/**
 * Maplecode is the main class for interacting with the Maplecode agent.
 *
 * Use the `startThread()` method to start a new thread or `resumeThread()` to resume a previously started thread.
 */
export class Maplecode {
  private exec: MaplecodeExec;
  private options: MaplecodeOptions;

  constructor(options: MaplecodeOptions = {}) {
    const { maplecodePathOverride, env, config, configOverrides } = options;
    this.exec = new MaplecodeExec(maplecodePathOverride, env, config, configOverrides);
    this.options = options;
  }

  /**
   * Starts a new conversation with an agent.
   * @returns A new thread instance.
   */
  startThread(options: ThreadOptions = {}): Thread {
    return new Thread(this.exec, this.options, options);
  }

  /**
   * Resumes a conversation with an agent based on the thread id.
   * Threads are persisted in ~/.maplecode/sessions.
   *
   * @param id The id of the thread to resume.
   * @returns A new thread instance.
   */
  resumeThread(id: string, options: ThreadOptions = {}): Thread {
    return new Thread(this.exec, this.options, options, id);
  }
}
