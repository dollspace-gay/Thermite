---
rfc: 5
title: An RFC process, and RFCs as files
status: draft
supersedes: []
introduces:
  - REQ-RFC-FRONTMATTER
  - REQ-RFC-GATE
  - REQ-RFC-REGISTRY-LINK
---

# An RFC process, and RFCs as files

This RFC proposes the process it is being submitted through, so its own PR is the
worked example.

## The problem

Three things that should reference each other live in three places that do not.

| | where it lives today |
|---|---|
| the **proposal** | a GitHub issue — RFC-1 is #2, RFC-2 is #119, RFC-3 is #120, and one is unnumbered |
| the **document** | `.design/` — `stage1-forge-tier.md`, `stage3-bv-reconstruction.md`, and the rest |
| the **requirements** | `.design/reqs/registry.toml`, generating a status table with evidence links |

An issue is the wrong primitive for the first of those. An issue is a **report**:
one author, one body, open or closed. A proposal needs versions, a status that is
not binary, and amendment history. On an issue, amendments become comments, so
the document a reader sees first is the stalest version of it.

The registry is the hard half and it already exists, from #17. What is missing is
the link in both directions: an RFC has no path to becoming tracked work, and a
REQ has no path back to the proposal that motivated it.

**#17 is the worked example of the whole problem.** It is an RFC that proposed
the canonical REQ registry, it shipped, and it never got a number — so it does
not appear in any sequence, its document is an issue body, and nothing connects
it to the requirements it created. An RFC that built the requirement-tracking
system is itself untracked.

## Proposal

RFCs become files.

```
.design/rfcs/0004-verified-effect-rows.md
```

```yaml
---
rfc: 4
title: Verified effect rows
status: draft | accepted | rejected | superseded
supersedes: []
introduces: [REQ-EFFECT-ROW-REGIONS, REQ-EFFECT-ROW-CONFLICT]
discussion: https://github.com/dollspace-gay/Thermite/issues/119
---
```

### One PR per RFC

Review happens line by line on the document rather than in a thread about a first
post that has since been superseded. The PR is the discussion until the RFC
lands.

### A draft's number is provisional; merging makes it canonical

Take the next free number when you write it, so the RFC is citable while it is
being argued about — "RFC-6" beats "the one in PR 214". The file is
`0006-slug.md` from the first commit.

**While `status: draft`, that number is provisional.** No extra field says so;
`draft` already means it. If two RFCs are drafted concurrently and collide, or a
reviewer wants a different slot, the number moves at merge and the file is
renamed in the merge commit. Once the status leaves `draft`, the number is
canonical and never moves again.

That is where the flexibility belongs. A number that is fixed the moment someone
opens a PR turns an ordering question into a race; a number that does not exist
until merge cannot be cited during the review that decides it.

The gate enforces the distinction rather than the etiquette:

| | |
|---|---|
| two non-draft RFCs share a number | **error** |
| a draft collides with a non-draft | **error** — the draft moves |
| two drafts share a number | reported, not fatal — merge resolves it |

Using the PR number instead was considered and rejected: it would jump the
sequence to a large sparse number interleaved with every other PR.

### Always merge, never close

A rejected RFC merges with `status: rejected`. Closing the PR instead would
discard the reasoning, which is the part worth keeping — the argument for why
something was *not* done is the expensive thing to reconstruct later.

### `introduces:` feeds the registry

The RFC declares the REQs it creates. The registry tracks whether they shipped.
The generated status table becomes the RFC's progress view without anyone
maintaining one.

So there is deliberately **no `implemented` status**. Whether an RFC is
implemented is computable — all of its REQs shipped — and a hand-maintained field
would drift from the registry, which is the failure the registry exists to
prevent.

### Versions are derived, not declared

One commit per edit of an RFC file, and a version is cited as:

```
RFC-4 r3 @ a1b2c3d
```

`r3` is the number of commits that have touched that file; `a1b2c3d` pins the
exact content. Both are computed:

```sh
git log --oneline -- .design/rfcs/0004-*.md | wc -l
```

So there is no `version:` field, for the same reason there is no `implemented`
status: a declared version drifts from the file it describes, and a derived one
cannot.

**`git log` on the file is the amendment history**, which is the thing an issue
cannot provide. An RFC that changed three times shows three diffs, each with a
message saying why. On an issue, an amendment is a comment and the reader has to
reconstruct the document's state at each point by hand.

One consequence to state rather than discover: **an RFC PR should not be
squash-merged**, or its review history collapses into one commit. The natural
reading is that r1 is the RFC as merged and later revisions are amendments, with
pre-merge iteration living in the PR — which works either way, but only if the
merge is not a squash when the iteration is worth keeping.

### `discussion:` names where the argument lives, when it is not here

Omitted for a new RFC, because its PR is the discussion and `git log` links the
two. Present on a migrated one, pointing at the issue it came from, so a
conversation in flight is not orphaned by the move.

## Migration

All three existing RFCs move, and the two live ones are the point.

| RFC | issue | state | what the move looks like |
|---|---|---|---|
| RFC-1 | #2 | closed | the terminal case: a document with a resolved status and an archived discussion |
| RFC-2 | #119 | open | a document now, its discussion still at #119 |
| RFC-3 | #120 | open | same |
and the sequence is renumbered so it is chronological:

| RFC | issue | filed | was | is |
|---|---|---|---|---|
| Thermite 2 | #2 | first | RFC-1 | RFC-1 |
| Canonical REQ registry | #17 | second | *unnumbered* | **RFC-2** |
| The certification surface | #119 | third | RFC-2 | **RFC-3** |
| Versioning | #120 | fourth | RFC-3 | **RFC-4** |

**Renumbering is done once, here, and never again.** It is possible now because
the cost is two issue titles: nothing in the tree references RFC-2 or RFC-3, and
no issue or PR body does either — checked. At any larger size it would not be
worth doing, and the convention from here is:

> **A number is identity.** Never reused, never reassigned. Chronology is visible
> from the git history and the linked issue.

Fixing the sequence before adopting that rule is cheaper than carrying an
anomaly under it forever.

**The migration is a text move, not an edit.** Issue bodies are copied verbatim
and front matter is added above them. Nothing is rewritten, reordered, or
summarised — an editorial pass would put words under an author's name that they
did not write.

**Comments stay in the issue.** The file is the document; the issue is the
discussion. That separation is the whole proposal, so applying it to the
migration is the consistent move rather than an omission. `discussion:` links
the two, and the issue links back.

An open RFC keeps its issue open. When its discussion resolves, the issue closes
pointing at the file and `status` moves off `draft`. Nobody has to move a thread
mid-argument.

## The gate

`tooling/rfc-check.py`, in the style of the existing gates: stdlib only, exit
non-zero with a specific message.

- every file in `.design/rfcs/` has valid front matter with the required fields
- `status` is one of the four values
- `rfc:` is unique, and matches the filename's numeric prefix
- every REQ in `introduces:` exists in `registry.toml`
- every `supersedes:` target exists

The script ships with this RFC. A process proposal that asks the maintainer to
write its own enforcement is one that does not land.

### The gate declares its interpreter

`rfc-check.py` carries a PEP 723 header, so `uv run tooling/rfc-check.py` fetches
a matching interpreter rather than inheriting whichever `python3` is on PATH.
`req-registry.py` and `reqs` get the same header, for a reason this PR ran into.

Those two parse the registry with `tomllib`, standard library from Python 3.11.
On an older interpreter they report

```
REQ registry inconclusive: tomllib is unavailable (Python < 3.11)
```

and exit **3** — so they fail rather than pass, which is right. What they do not
do is tell you anything about the registry, and the environment error stands in
front of whatever the real finding was. In this PR the real finding was a fault
of mine: three requirements added to the registry without regenerating the
status view they appear in. It was invisible until the gate ran on an
interpreter that could parse the file.

An inconclusive gate is not a lie, but it is a result nobody can act on. The
header makes `uv run tooling/reqs check` produce the actual verdict.

## What this does not change

**Issues stay defect reports**, which is what they are good at. #122–#126 are
right where they belong.

**`.design/` documents stay.** An RFC proposes a change; a design document
governs a component. They are different artifacts, and the RFC can reference the
document it will produce or amend.

**No approval ceremony is added.** There is no review period, no shepherd, no
FCP. The proposal is a file and a PR, and the existing review is the review.

## Why files, beyond the immediate

A file with front matter is a record with fields. Whatever this eventually
federates to — an atproto lexicon, a static site, a generated index — reads a
directory of documents. An issue thread has no such shape, and a migration
later is strictly harder than starting here.

## Self-hosting

This RFC is `.design/rfcs/0005-rfc-process.md`, numbered like any other, in a PR
that is its own discussion. If the process is wrong, the artifact demonstrating it is the thing
being reviewed, which is the cheapest possible way to find out.
