// Shared `fsm` CLI process glue for the V3 CLI-wrapper commands (Doc 27
// §8-V3 / §5 "thin glue over the `fsm` CLI; none re-implements analysis").
//
// WHY a single runner: every V3 CLI command (check / generate / fmt / IR)
// is "spawn the real `fsm`, capture exit+stdout+stderr, surface the result
// HONESTLY". Centralising it means the cardinal-sin bar — never present a
// failed CLI run as success — is enforced in exactly one place. The runner
// NEVER swallows a non-zero exit: callers decide what a given exit means
// (e.g. `fsm check` exits 1 with diagnostics, that is not a command
// failure), but the raw exit/stderr is always returned, never hidden.

import { execFile } from "child_process";

export interface CliResult {
  /** Process exit code (or a synthetic non-zero if it could not spawn). */
  readonly code: number;
  readonly stdout: string;
  readonly stderr: string;
  /** True iff the process could not even be spawned (e.g. ENOENT). */
  readonly spawnError: boolean;
}

export interface RunCliOptions {
  /** stdin to write to the child (used by `fsm fmt --stdin`). */
  readonly stdin?: string;
  /** Working directory for the child (used to root relative outputDir). */
  readonly cwd?: string;
}

/**
 * Spawn the `fsm` CLI with `args`, await completion, and return the raw
 * exit code + captured streams. Resolves (never rejects) for a process that
 * ran — a non-zero exit is data, not an exception (the caller maps it to a
 * user-honest outcome). Only a genuine spawn failure (binary missing /
 * not executable) sets `spawnError`.
 */
export function runCli(
  cliPath: string,
  args: string[],
  opts: RunCliOptions = {},
): Promise<CliResult> {
  return new Promise<CliResult>((resolve) => {
    const child = execFile(
      cliPath,
      args,
      { cwd: opts.cwd, maxBuffer: 32 * 1024 * 1024 },
      (error, stdout, stderr) => {
        if (error && (error as NodeJS.ErrnoException).code === "ENOENT") {
          resolve({
            code: -1,
            stdout: stdout ?? "",
            stderr: stderr ?? String(error),
            spawnError: true,
          });
          return;
        }
        // `error` is set for any non-zero exit too; recover the real code
        // from it rather than masking the failure as success.
        const code =
          error && typeof (error as { code?: number }).code === "number"
            ? ((error as { code?: number }).code as number)
            : 0;
        resolve({
          code,
          stdout: stdout ?? "",
          stderr: stderr ?? "",
          spawnError: false,
        });
      },
    );

    if (opts.stdin !== undefined && child.stdin) {
      child.stdin.end(opts.stdin);
    }
  });
}
