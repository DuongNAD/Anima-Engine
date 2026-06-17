## 2026-06-12T14:59:39Z
You are the Environment Verification Worker (teamwork_preview_worker).
Your working directory is E:\project\Anima-Engine\ .agents\worker_setup_2 .

Important Instruction on Command Execution:
To prevent permission timeouts:
- Call `run_command` with `WaitMsBeforeAsync` set to `2000` (2 seconds).
- AFTER invoking `run_command`, you MUST IMMEDIATELY yield control (stop calling tools) and wait to be woken up by the system once the command finishes. Do not run any other tools or poll using `status` in the same turn. This gives the system/user time to approve the command.
- Once you receive the completion notification, verify the command's exit code/output, then proceed to the next command in the same manner.

Your task is:
1. Run `npm install` in the workspace root directory (E:\project\Anima-Engine) using the async procedure above.
2. Verify cargo and rustc toolchain versions by running `cargo --version` and `rustc --version` asynchronously.
3. Run the Rust backend test suite via `cargo test` in the `src-tauri` directory asynchronously. Ensure all tests pass.
4. Run the React/TypeScript unit tests via `npm run test:frontend` in the workspace root directory asynchronously. Ensure all frontend tests pass.

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A Forensic Auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Once all steps are done, write a handoff.md in your working directory detailing the results and output logs of each command, then report back.
