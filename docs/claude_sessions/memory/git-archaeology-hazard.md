---
name: git-archaeology-hazard
description: Testing old commits via git checkout can detach HEAD and orphan the commit chain — recover via reflog
metadata: 
  node_type: memory
  type: feedback
  originSessionId: 8f7b8292-fbc0-48f8-8506-8b6a0949123b
---

Running "was this failing before?" archaeology with `git checkout <old-sha>` **detaches HEAD**. If you then `git checkout "$(git rev-parse --abbrev-ref HEAD)"` while already detached, `--abbrev-ref HEAD` returns the literal string `HEAD` (not `master`), so the checkout-back is a no-op and you stay detached. Subsequent commits pile onto the OLD base, orphaning the whole real chain, and `git push origin master` pushes the stale `master` branch (a no-op) while your work never reaches origin — even though each push prints success.

**Why:** on a detached HEAD, branch refs don't move with your commits.

**How to apply:**
- To test an old commit safely: `git stash -u`, `git checkout -q <sha>`, run, then `git checkout -q master` (by NAME, never via `--abbrev-ref`), `git stash pop`. Verify `git log --oneline -1` shows your tip afterward.
- Prefer NOT checking out at all: `git show <sha>:path/to/file` reads a file at any commit; `git merge-base --is-ancestor` checks lineage — both without moving HEAD. Or use a worktree.
- **Recovery** if orphaned: `git reflog` shows the real chain tip (look for the last `commit:` before the stray `checkout:` lines). `git checkout -f -B master <good-tip-sha>` re-points master and discards the wrong-base tree; back up any uncommitted work first (files whose base is unchanged copy over cleanly). Confirm origin/master is an ancestor of the good tip (`git merge-base --is-ancestor origin/master <tip>`) so the push fast-forwards.

Happened 2026-07-04: the whole oracle-audit chain (a0bc38e tip) got orphaned; recovered from reflog, no work lost. See [[rom-oracle-plan]].
