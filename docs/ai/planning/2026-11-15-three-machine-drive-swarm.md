---
title: Swarm ba máy giao tiếp qua Google Drive — giao thức và prompt
status: proposed
owner: maintainers
last_reviewed: 2026-11-15
review_cycle: weekly
---

# Swarm ba máy giao tiếp qua Google Drive

> Tài liệu này **không** thay thẻ, luật hay quyết định nào. Thẻ, PROMPT 0, BLOCK S và VERIFIER vẫn ở
> [`2026-11-05-swarm-prompts.md`](2026-11-05-swarm-prompts.md); BLOCK 0 vẫn ở
> [brief 31/10](2026-10-31-antigravity-brief.md); quyết định vẫn ở
> [roadmap 31/10 §4](../../planning/COMPLETION_ROADMAP_2026-10-31.md#4-quyết-định-chỉ-chủ-dự-án-đưa-ra-được).
> Ở đây chỉ thêm **một lớp vận chuyển**: ba máy, mỗi máy một agent Gemini Carbon trong Antigravity,
> nói chuyện với nhau qua một thư mục Google Drive dùng chung.
>
> Prompt viết bằng tiếng Anh, theo cùng quy ước với file prompt swarm. Phần giải thích viết bằng
> tiếng Việt.

## 1. Thiết kế trong năm dòng

1. **Git là đường duy nhất chở mã.** Drive chỉ chở *tin nhắn, trạng thái và log*. Repo và worktree
   **không bao giờ** nằm trong thư mục Drive: Drive đồng bộ `.git/` và `target/` sẽ hỏng cả hai.
2. **Mỗi file trên Drive có đúng một người ghi.** Drive không có khoá. Hai máy sửa cùng một file thì
   Drive sinh bản `tên (1).md` và một bên mất dữ liệu. Vì vậy tin nhắn là file **mới, bất biến**.
   Trạng thái thì mỗi máy chỉ ghi file của chính nó.
3. **Máy A là lead và không build Rust.** Đó là luật L9 của
   [kế hoạch 13/11](../../planning/DIRECTION_AND_COORDINATION_PLAN_2026-11-13.md): khi đã có máy thứ
   hai, build nặng không chạy trên máy phát triển. **B và C là trạm làm việc**, mỗi trạm chạy một thẻ
   một lúc.
4. **Verify chéo máy.** Thẻ làm trên B thì verify trên C, và ngược lại. Verifier luôn là một cuộc hội
   thoại **mới**, đúng như §3 của file prompt swarm.
5. **Chủ dự án vẫn là người duy nhất merge.** Agent không merge, không đóng PR và không tự giao thẻ
   cho mình. Trạm không tự nhận việc khi lead chưa giao.

## 2. Chuẩn bị một lần (chủ dự án)

### 2.1 Việc phải xong trước khi bật Đợt 2

| # | Việc | Vì sao chặn |
|---|---|---|
| 1 | Đẩy bản sửa clippy lên thân cây: 32 lint trong 8 file test, commit `b1953e4` ngày 2026-11-15 | Nếu chưa merge, mọi PR của Đợt 2 sẽ đỏ ở `cargo clippy` và ở bước tách feature. Verifier khi đó không phân biệt được đỏ mới với đỏ cũ |
| 2 | Quyết định 4 manifest E2 bị sửa ở `970d963` (thêm `live.dormant_eu`): hoàn nguyên fixture về bản đã đăng ký trước, hay đăng ký lại | `npm run check:e2` đỏ trên thân cây. Đây là quyết định về bản đăng ký trước, máy không chọn được |
| 3 | Điền các dòng `R5-AUTH` của Đợt 2 (mẫu ở §7) | G1 và G3 ghi vào `src-tauri/src`. Theo QĐ-B, thiếu dòng này thì thẻ không chạy |
| 4 | Cập nhật [AGENT_WORKFLOW](../../how-to/AGENT_WORKFLOW.md) §1 và §3: Đợt 1 đã merge (#42–#45) | Lead đọc trang đó để biết đang ở đâu |

Mục 2 chưa xong thì vẫn chạy được. Khi đó lead ghi `check:e2` vào danh sách **đỏ nền** ở `BOARD.md`
(bước L2 của PROMPT L), và không quy lỗi đó cho worker nào.

### 2.2 Mỗi máy

1. Cài Google Drive for desktop và đăng nhập **cùng một tài khoản** trên cả ba máy. Tạo thư mục
   `AnimaSwarm` trong My Drive. Bấm chuột phải → **Available offline** (hoặc chọn chế độ *Mirror
   files*). Ở chế độ *Stream*, file chỉ tải về khi mở, nên một máy có thể thấy tên tin nhắn mà
   chưa đọc được nội dung.
2. Đường dẫn cục bộ của thư mục (ghi vào khối `MACHINE` ở §4):
   - macOS: `~/Library/CloudStorage/GoogleDrive-<tài khoản>/My Drive/AnimaSwarm`
   - Windows: `G:\My Drive\AnimaSwarm` (chữ ổ mặc định của Drive for desktop)
   - Linux: Google không có client chính thức. Dùng `rclone bisync`, và nhớ rằng độ trễ lúc đó là
     chu kỳ đồng bộ của bạn.
3. Clone repo **ngoài** thư mục Drive, `git fetch origin`, đăng nhập `gh` (cần để mở draft PR).
   Máy B và C cần toolchain Rust (cài theo `rust-toolchain.toml`), Node, và Playwright nếu sẽ chạy G2.
4. Cho Antigravity quyền đọc và ghi thư mục Drive: thêm nó làm thư mục thứ hai của workspace, hoặc
   cho phép terminal truy cập đường dẫn đó.
5. Chọn model **Gemini Carbon** cho mọi cuộc hội thoại của swarm.

### 2.3 Dựng thư mục Drive

```text
AnimaSwarm/
  PROTOCOL.md            chủ dự án — dán nguyên văn BLOCK D (§3)
  DECISIONS.md           chủ dự án — khối DECISIONS (§7)
  BOARD.md               chỉ lead (A) ghi — bảng giao việc
  machines/A/  machines/B/  machines/C/
      STATUS.md          chỉ máy đó ghi — nhịp tim và trạng thái
      SEEN.md            chỉ máy đó ghi — tên các tin nhắn đã xử lý
  msgs/                  tin nhắn: mỗi tin một file, không ai sửa hay xoá
  logs/<CARD>/           output thô của lệnh, mỗi lần chạy một file
```

Tạo sẵn các thư mục rỗng. Chủ dự án cũng gửi tin qua `msgs/` với `FROM = OWNER`.

## 3. BLOCK D — giao thức Drive (dán cho mọi agent, và lưu thành `PROTOCOL.md`)

```text
BLOCK D — DRIVE PROTOCOL. These rules add to BLOCK 0 and BLOCK S; where they differ, the
stricter rule wins. <DRIVE> below is the local path of the shared AnimaSwarm folder given in
your MACHINE block.

D1  TRANSPORT. Git and GitHub carry code; Drive carries coordination only (messages, status,
    logs). Never place a clone, a worktree, target/, node_modules/ or any build output inside
    <DRIVE>. Never copy source files into <DRIVE>; refer to them by branch, sha and path.

D2  ONE WRITER PER FILE. You may write only:
      <DRIVE>/machines/<YOUR-ID>/STATUS.md and <DRIVE>/machines/<YOUR-ID>/SEEN.md
      new files in <DRIVE>/msgs/ whose name has FROM = <YOUR-ID>
      new files in <DRIVE>/logs/<CARD>/ whose name starts with <YOUR-ID>
      <DRIVE>/BOARD.md, only if your ROLE is LEAD
    You never edit, rename or delete any other file. PROTOCOL.md and DECISIONS.md belong to the
    owner. A file you did not write is read-only for you, even if it looks wrong: report it.

D3  MESSAGE NAME. msgs/<UTC>__<FROM>__<TO>__<TYPE>__<CARD>.md
      UTC   = yyyymmddTHHMMSSZ from your clock in UTC, e.g. 20261115T093000Z
              (macOS/Linux: date -u +%Y%m%dT%H%M%SZ;
               PowerShell: (Get-Date).ToUniversalTime().ToString("yyyyMMddTHHmmssZ"))
      FROM  = A | B | C | OWNER          TO = A | B | C | OWNER | ALL
      TYPE  = ASSIGN | VERIFY | ACCEPT | PROGRESS | REPORT | VERDICT | BLOCKED | QUESTION
              | ANSWER | STOP | MERGED | BOARD
      CARD  = the card ID, or NONE
    ASCII letters, digits, '-', '_' and '.' only. No spaces and no ':' (illegal on Windows).

D4  MESSAGE BODY. Exactly this header, then the body, then the END line:
      FROM: <id>
      TO: <id>
      TYPE: <type>
      CARD: <id or NONE>
      REPLY-TO: <file name of the message this answers, or none>
      BASE: <sha or none>   HEAD: <sha or none>   BRANCH: <name or none>   PR: <url or none>
      ---
      <body>
      -- END --
    A file whose last non-empty line is not "-- END --" has not finished syncing: skip it and
    read it again on the next poll. Never act on a partial message.

D5  ATOMIC WRITE. Write the message in a temporary file OUTSIDE <DRIVE>, check that it ends
    with "-- END --", then move it into <DRIVE>/msgs/ in one operation. Same for logs.

D6  READING. Poll <DRIVE>/msgs/ every 3 minutes while you wait, and before every state change.
    Process files whose TO is your ID or ALL, in file-name order, skipping names listed in your
    SEEN.md. After acting on one, append its name to SEEN.md. Clocks differ between machines:
    never let correctness depend on two machines' timestamps; follow REPLY-TO chains instead.

D7  CONFLICT COPIES. A name containing " (1)", "(conflict" or any name that breaks D3 is a sync
    conflict. Do not act on it, delete it or rename it. Send a QUESTION to A and OWNER with the
    exact name.

D8  STATUS. Keep <DRIVE>/machines/<YOUR-ID>/STATUS.md in this form, rewritten whole each time:
      ID: <id>    ROLE: LEAD | STATION    OS: <os>
      STATE: IDLE | WORKING | VERIFYING | BLOCKED | WAITING_OWNER | OFFLINE
      CARD: <id or none>   BRANCH: <name or none>   WORKTREE: <path or none>
      LAST COMMAND: <command>  EXIT: <code>
      UPDATED: <UTC>
    Rewrite it on every state change and at least every 15 minutes while WORKING or VERIFYING.
    Set OFFLINE before you stop on purpose.

D9  LOGS. Every DONE WHEN command and every verifier command writes its raw output, both
    streams in write order, to logs/<CARD>/<YOUR-ID>__<UTC>__<short-name>.txt. A report cites
    the log file name and also pastes the summary lines it relies on. Never edit a log after
    writing it; rerun and write a new one. On Windows capture with
      cmd /c "cargo test ... > out.txt 2>&1"
    never a PowerShell pipeline (it reorders cargo's two streams).

D10 SECRETS. Nothing that holds a credential goes to <DRIVE>: no .env, API key, token,
    password, SSH key or `gh auth` output. Before moving any file into <DRIVE>, run
      grep -nE 'API_KEY|TOKEN|SECRET|PASSWORD|ghp_|github_pat_|AIza|BEGIN [A-Z ]*PRIVATE KEY'
    on it; if anything matches, do not move it, and report the match without its value.

D11 HANDS OFF. Never start work that no ASSIGN or VERIFY addressed to you asked for. Never
    reassign, resume or push to a branch another machine created. A machine that has gone
    silent is the owner's problem, not yours: report it.

D12 OUTAGE. If <DRIVE> is unreachable or stops syncing, keep your git work, stop sending, set
    nothing, and tell the human at your keyboard. Do not invent a second channel.
```

## 4. Khối `MACHINE` — mỗi máy một bản, dán ngay sau prompt vai

```text
MACHINE
  ID         = A | B | C
  ROLE       = LEAD (A) | STATION (B, C)
  OS         = macOS 27 arm64 | Windows 11 x64 | ...
  REPO       = <absolute path of the clone, outside DRIVE>
  WORKTREES  = <absolute path of the folder for worktrees, outside DRIVE>, e.g. ../anima-wt
  DRIVE      = <absolute path of AnimaSwarm on this machine>
  CARGO JOBS = 2      (BLOCK S S6 default; the owner may raise it on a dedicated machine)
  MAY RUN    = [RUST] and non-[RUST] cards (B, C) | non-[RUST] cards only, no cargo (A)
```

## 5. PROMPT L — lead, máy A

Dán theo thứ tự: **PROMPT 0** (§1 file prompt swarm, khối `DECISIONS` đã điền như §7), rồi
**BLOCK D**, rồi khối dưới đây, rồi khối `MACHINE` của A.

```text
LEAD ADDENDUM FOR THREE MACHINES — this replaces PROMPT 0 steps 4, 5 and 6 and adds L1-L9.
Everything else in PROMPT 0 stands: you plan, dispatch and audit; you never write production
code, never merge, never edit a worker's branch.

L1  SETUP. Read <DRIVE>/PROTOCOL.md and <DRIVE>/DECISIONS.md. They win over anything pasted
    into this conversation if the two differ; say so if they do. Re-read DECISIONS.md at the
    start of every loop: the owner edits it.

L2  PREFLIGHT. Do PROMPT 0 step 1. Also record the trunk sha and, from
      gh run list --branch <trunk-head-branch-or-PR> / gh run view <id> --json jobs
    the CI steps that are already RED at that sha. Write them into BOARD.md as BASELINE RED,
    with the run id. A worker is never blamed for a BASELINE RED step, and a BASELINE RED step
    never counts as a pass.

L3  BOARD. BOARD.md is yours alone. Rewrite it whole after each change:
      TRUNK <sha> · WAVE <n> · UPDATED <UTC>
      BASELINE RED: <step> (run <id>), ...
      | card | machine | state | branch | PR | verifier machine | verdict | blocker |
      MACHINES: A <STATE, UPDATED> · B <...> · C <...>   (copied from their STATUS.md)
    After rewriting it, send a TYPE BOARD message to ALL whose body is the new board.

L4  CAPACITY. One card per station at a time. [RUST] cards run only on B or C, never on A.
    Your own machine runs no cargo command at all. Keep PROMPT 0 step 3: two cards whose
    ALLOWED PATHS overlap, or that both list a shared path, never run at the same time, even
    on different machines.

L5  DISPATCH. To give a card to a station, send ASSIGN to that station. Its body is the
    complete worker prompt, verbatim, in this order: BLOCK 0, BLOCK S, BLOCK D, the card (plus
    the named brief section for (brief) cards), the R5-AUTH line for this card if any, and the
    line "BASE-SHA = <trunk sha>". Copy text with git show; never retype or shorten it. Wait
    for ACCEPT or BLOCKED; with neither after 30 minutes, mark the card UNACKED on the board
    and send a QUESTION to OWNER. Never send one station two cards at once.

L6  VERIFY. When a REPORT arrives, send VERIFY to the OTHER station (work from B is verified
    on C, from C on B; a card done on A is verified on B or C). Body, verbatim: the VERIFIER
    TEMPLATE, BLOCK D, the card, the worker's REPORT message in full, and the branch name.
    If the other station is busy, wait; never verify on the machine that did the work, and
    never verify yourself. If the two machines run different OSes and the verdict turns on an
    OS difference, record it as a finding for the owner rather than a MISMATCH.

L7  VERDICTS. PROMPT 0 step 7 stands: accept only VERIFIED; send a MISMATCH back to the same
    station once, as a new ASSIGN that quotes the verifier's list; a second MISMATCH stops the
    card and goes to OWNER. PROMPT 0 step 8 (stop the wave) also applies to anything a VERDICT,
    REPORT or log on Drive shows.

L8  OWNER EVENTS. On MERGED from OWNER, prove it with git merge-base --is-ancestor, update
    TRUNK on the board, and re-check every card's eligibility (PROMPT 0 step 2): a card whose
    DEPENDS ON just became true may now be dispatched. On STOP from OWNER, send STOP to ALL
    and dispatch nothing more.

L9  SILENCE. A station whose STATUS.md UPDATED is older than 45 minutes while WORKING or
    VERIFYING is STALLED: mark it on the board and send a QUESTION to OWNER. Never reassign
    its card: a second writer on the same branch is worse than a delay.

STATUS REPORT: PROMPT 0's table, posted as BOARD (L3), and repeated to OWNER as a REPORT
message at the end of each wave.
```

## 6. PROMPT W, PROMPT V, PROMPT R — trạm B và C

### PROMPT W — trạm (dán ở B và C, kèm BLOCK D và khối `MACHINE`)

```text
ROLE
You are STATION <ID> of a three-machine swarm on the Anima-Engine repository. You do not
plan or choose work. The lead (machine A) sends you work as messages on Drive. Read BLOCK D
below and <DRIVE>/PROTOCOL.md; if they differ, stop and tell the human at your keyboard.

LOOP
1. Write STATUS.md (STATE IDLE). Poll <DRIVE>/msgs/ as D6 says.
2. On ASSIGN for you:
   a. If this conversation has already worked a card or run a verification, do not start
      it here. Start a NEW agent conversation (Antigravity Agent Manager) whose whole first
      message is the ASSIGN body verbatim followed by BLOCK D and your MACHINE block, and
      keep this conversation as the watcher. If you cannot start a new conversation, set
      STATE WAITING_OWNER and send QUESTION to OWNER: "open a new conversation on <ID> with
      PROMPT W for card <CARD>".
   b. Otherwise, this conversation becomes the worker: send ACCEPT (or BLOCKED with the
      reason, per BLOCK S S4/S5), set STATE WORKING, and do the card exactly as the ASSIGN
      body says, in a worktree under WORKTREES.
   c. Send PROGRESS only when a DONE WHEN command finishes or you hit a blocker: one line
      plus the log name.
   d. Finish with REPORT to A: the BLOCK S S8 report in full, every log name it relies on,
      and the PR url. If DONE WHEN did not exit 0, the first word of the body is NOT DONE.
3. On VERIFY for you: same rule as 2a (verification always runs in a conversation that has
   seen neither the work nor another verification). The verifier sends VERDICT to A.
4. On STOP: finish the command that is running, commit nothing new, push nothing, set STATE
   OFFLINE, and wait for the human.
5. Never send REPORT for work you did not do, never send VERDICT for work you did.
```

### PROMPT V — verifier

Không có prompt riêng. Thân của tin `VERIFY` đã là prompt đầy đủ (VERIFIER của file prompt swarm,
BLOCK D, thẻ, báo cáo, nhánh). Trạm dán nguyên văn thân tin đó vào một cuộc hội thoại mới, thêm
khối `MACHINE`, rồi gõ thêm một dòng:
`When done, send VERDICT to A as BLOCK D says; your first line is the verdict.`

### PROMPT R — tiếp tục sau khi một phiên chết

```text
RESUME. You are <ROLE> <ID>; your MACHINE block and BLOCK D follow. A previous conversation in
this role stopped. Rebuild state before doing anything:
1. Read <DRIVE>/PROTOCOL.md, DECISIONS.md, BOARD.md, machines/<ID>/STATUS.md and SEEN.md.
2. List msgs addressed to <ID> or ALL that are not in SEEN.md; do not act on them yet.
3. If STATUS names a WORKTREE: cd there, run git status, git branch --show-current,
   git log --oneline -5, and git log origin/<branch> --oneline -1. Pushed commits are done;
   never redo or rewrite them.
4. Write a one-paragraph summary of where the work stands, send it as PROGRESS to A (or, if
   you are A, as BOARD to ALL), then continue the loop of your role from that point.
```

## 7. Khối `DECISIONS` cho Đợt 2 — lưu thành `DECISIONS.md`

Lấy khối ở [AGENT_WORKFLOW §4](../../how-to/AGENT_WORKFLOW.md#4-khối-decisions-để-dán-vào-prompt-0)
rồi đổi ba chỗ sau. Mọi ô khác giữ nguyên, kể cả các ô trống: thẻ nào cần ô trống thì lead sẽ từ chối
thẻ đó.

```text
CURRENT WAVE = 2
MACHINES     = A lead (no cargo) · B station · C station
QD-E2M       =            (owner: E2 fixture manifests — revert to the preregistered bytes,
                           or re-register; until filled, check:e2 is BASELINE RED)
R5-AUTH      =
  <keep the T1 and D0 lines>
  AUTHORIZATION (owner, <YYYY-MM-DD>): for card G1 only, you may write to
  src-tauri/src/core/agent_systems.rs, src-tauri/src/core/mortality.rs,
  src-tauri/src/core/spawn.rs. This overrides AGENTS.md's read-only declaration and BLOCK 0
  R5 for exactly these paths and no others.
  AUTHORIZATION (owner, <YYYY-MM-DD>): for card G3 only, you may write to
  src-tauri/src/core/telemetry_export.rs, src-tauri/src/commands/simulation.rs,
  src-tauri/src/core/energy*.rs. This overrides AGENTS.md's read-only declaration and BLOCK 0
  R5 for exactly these paths and no others.
```

Hai dòng `AUTHORIZATION` ở trên là **bản nháp**. Chỉ khi chủ dự án điền ngày và dán chúng vào
`DECISIONS.md` thì chúng mới là uỷ quyền. Đường dẫn chép đúng từ `ALLOWED PATHS` của thẻ, chỉ phần
nằm dưới `src-tauri/src`. G7 chỉ cần một dòng như vậy nếu nó phải sửa `src-tauri/src/bin/anima-lab.rs`.

**Thẻ chạy được khi các điều kiện ở §2.1 đã xong** (kiểm lại ngày 2026-11-15):

| Thẻ | Máy | Chặn bởi |
|---|---|---|
| G1 `[RUST]` | B | dòng `R5-AUTH` của G1 |
| G3 `[RUST]` | C | dòng `R5-AUTH` của G3 |
| G2 | A hoặc trạm còn rảnh | — (đụng `ci.yml`, nên không chạy cùng G9) |
| G7 `[RUST]` | trạm rảnh đầu tiên sau G1/G3 | — (cần `R5-AUTH` nếu sửa `anima-lab.rs`) |
| G5 `[RUST]` | — | G1 phải merge trước (cùng sửa `agent_systems.rs`) |
| G4, G6 | — | QĐ-2/QĐ-H trống |
| G9 | — | G6 |
| G8 | — | phần lớn đã làm; còn chờ chủ dự án giải xung đột `PROJECT.md` mà T2 để lại |

## 8. Thứ tự bật

1. Máy A: dán **PROMPT L**. Đợi `BOARD.md` đầu tiên xuất hiện và đọc mục `BASELINE RED`.
2. Máy B và C: dán **PROMPT W**. Mỗi trạm ghi `STATUS.md` với `STATE IDLE`.
3. Lead gửi `ASSIGN`. Đọc bảng trong `BOARD.md` thay vì đọc từng cuộc hội thoại.
4. Khi một thẻ có `VERIFIED`: tự đọc diff, merge PR, rồi gửi tin
   `…__OWNER__ALL__MERGED__<CARD>.md` có `HEAD: <merge sha>`. Lead tự mở khoá các thẻ phụ thuộc.
5. Muốn dừng tất cả: gửi tin `…__OWNER__ALL__STOP__NONE.md`.

## 9. Bẫy đã biết

- **Độ trễ của Drive là vài giây tới vài phút**, và có thể lâu hơn khi máy ngủ. Giao thức không cần
  nhanh. Nó cần không mất tin, nên mọi bước đều chờ một tin có `-- END --`.
- **Hai máy khác hệ điều hành cho kết quả khác nhau là chuyện có thật** (B2 chính là việc sửa một test
  allocator chỉ hỏng trên macOS). L6 bắt lead ghi khác biệt đó thành finding cho chủ dự án, không
  để worker "sửa cho khớp".
- **Một agent có thể tự sửa `BOARD.md` hay `DECISIONS.md` vì thấy chúng "sai".** D2 cấm việc này. Nếu
  vẫn thấy một file như vậy đổi mà không có tin `BOARD` tương ứng, hãy dừng đợt và đọc lịch sử phiên
  bản của file trên drive.google.com.
- **Antigravity có thể không cho một agent mở cuộc hội thoại mới.** Khi đó trạm chuyển sang
  `WAITING_OWNER` (bước 2a của PROMPT W), và bạn phải tự mở cuộc hội thoại mới trên máy đó. Đổi lại,
  verifier luôn có ngữ cảnh sạch.
