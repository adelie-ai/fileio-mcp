# Path safety

`PathGuard` decides which filesystem paths this server may reach. It is an
allowlist. A small set of roots is reachable. Every other path does not exist,
as far as this server is concerned.

## Two checks, not one

The guard runs in two places:

- on every path argument a tool receives, before the operation runs;
- on every path a tool is about to return, before the result leaves the server.

Both are needed. An argument check on its own lets a listing of a permitted
directory disclose a path outside the set. A symlink inside the root, or a
subtree the operator subtracted, is enough to do it.

## Where the roots come from

The first source that gives a non-empty set wins:

1. `--allow-path <path>` on the command line. Repeat the flag for more roots.
   Give an absolute path. A relative one resolves against the directory the
   server was started in, so the allowlist would change with the launch site.
2. `FILEIO_MCP_ALLOW_PATHS`, a `:`-separated list of roots. A root whose own
   name contains `:` cannot be named this way; use `--allow-path` for it.
3. The built-in default set.

The built-in default set is:

- the system temporary directory (`TMPDIR`, usually `/tmp`);
- `~/Documents`, `~/Downloads`, `~/Desktop` and `~/Projects`, when `HOME` is set.

The default is a starting point for a desktop install, not a recommendation.
Name the directories the work actually needs and set them explicitly.

The override matters for tests as much as for operators. A test run sets
`FILEIO_MCP_ALLOW_PATHS` to a temporary directory it made to be thrown away, or
builds the guard with `PathGuard::with_roots`. It then cannot reach a real home
directory by construction, whatever the code under test does.

A root is a directory prefix. A path equal to a root, or below it, is inside the
set.

## Symlinks and `..`

The guard resolves a path first and compares it to the roots second. This order
is the whole point. A guard that compares first is defeated by
`/root/../etc/passwd`, and by a symlink in `/root` that points at `/etc`.

Resolution walks the path one component at a time, starting at `/`:

- `.` is dropped.
- `..` removes the last resolved component. At `/` it does nothing, as in the
  kernel.
- A component that is a symlink is replaced by its target, and the target is
  then resolved in turn. An absolute target restarts the walk at `/`. A cap of
  40 link hops stops a loop.
- A component that does not exist is kept as written. A path that does not exist
  yet must still be checked, because a write creates it. A component that does
  not exist cannot be a symlink, so nothing is missed.

A leading `~` expands first, from `HOME`, and so does any `$VAR`. The guard
expands exactly what the operations expand (`shellexpand::full`). Expanding less
would let a caller name one path to the guard and a different one to the
operation: `$HOME/.ssh/id_rsa` is a relative name to a guard that expands only
`~`, and an absolute path to the operation that opens it. An expansion that
fails, such as an undefined variable, names no path the guard can identify, so
it refuses.

The result is absolute, and free of symlinks, `.` and `..`. It is compared to
each root by whole path components, so `/home/user/documents-private` is not
inside the root `/home/user/documents`.

## Fail closed

The guard refuses anything it cannot positively identify:

- An empty root set refuses every path.
- A path the guard cannot resolve is refused. `lstat` on a component fails with
  `EACCES` when a parent directory is unreadable, so the guard cannot tell
  whether that component is a symlink. It refuses instead of guessing.
- A root the guard cannot resolve is dropped when the guard is built, rather
  than kept as a string that might match by accident.
- A result entry the guard cannot resolve is dropped from the result.

## The check-then-use race is open

The guard resolves the path, decides, and then the operation opens the path
again by name. Between the two, another process can replace a component with a
symlink that points out of the root. The guard does not close this race.

Closing it means opening each component with `openat` and `O_NOFOLLOW` from a
directory handle held across the check, and acting on that handle instead of on
the path. That is a rewrite of every operation under `src/operations`, tracked
as issue #26.

The exposure is small for the deployment this server targets. It runs as the
local user and serves that user's own agent. An attacker who can create a
symlink inside an allowed root at the right moment already has write access as
that user, and can read the target file directly.

## Refusal looks like absence

Every refusal, on an argument or on a result, looks like an absent file:

- a refused argument returns `File not found: <path>`;
- a refused result entry is dropped from the listing, with no gap and no marker;
- a tool whose whole result is one path, such as
  `fileio_get_current_directory`, returns `File not found` when that path is
  outside the set.

The server never answers "permission denied", and never names the allowlist. A
caller cannot tell a refusal from an empty directory.

An earlier design went further. A write to a blocked path reported success and
did nothing, and several tools built synthetic results so that a block matched
the shape of a real answer. That design is retired. It existed because a
deny-list is a map of the secrets it hides, so the list itself had to stay
invisible. An allowlist is not a map of anything. It names the working
directories the operator chose, and the operator can tell the model what they
are. The cost of the old design was real, because a write that vanished looked
to the model like a saved file, and work was lost.

## A refused path refuses the whole call

Several tools take an array of paths. When the guard refuses one of them, the
whole call is refused and the message names that path. Running the operation on
the rest gives a partial answer that the caller cannot tell from a complete one.
Split the call instead.

## What resolution cannot see

Resolution follows symbolic links, `.` and `..`. It cannot see a hard link,
because a hard link has no target: it is a second name for the same file. A
hard link inside a root, made earlier and pointing at a file outside every
root, resolves to a path inside the root and reads clean. No path-based guard
can tell the difference, and this one does not try.

`SECURITY_AUDIT.md` records that neither end of a link this server creates may
leave an allowed root. That covers links this server makes, not links it finds.

## Where the operation acts is not always where the argument points

The guard decides about a path, and the operation then acts. The two must agree
about which path that is:

- Expansion. The guard expands with `shellexpand::full`, and so does every
  operation, including the ones that collect a copy, move or remove source.
- A symbolic link points at the expanded target, not at the text the caller
  typed, so the link the server makes is the one the guard approved.
- `mktemp` creates in the template's parent directory. A template with no
  separator has an empty parent, which is the working directory and not the
  temporary directory, so that is what gets checked.

## Disclosure is a separate question from access

`fileio_read_symbolic_link` returns the text a link holds. Access and
disclosure ask different questions of it. Access follows the whole chain, so a
link whose chain ends inside a root is reachable. Disclosure is about the text:
a target reading `/elsewhere/back/notes.md` names `/elsewhere`, whatever it
resolves to. The returned text is normalized without following anything, and
refused when it names a path outside the set.

## A glob is checked by what it matches

`fileio_copy`, `fileio_move` and `fileio_remove` accept a glob in place of a
path. The pattern as written always stays inside the root that contains it, so
it says nothing about the entries it matches: a symlink among them can point out
of the root, and a copy follows it. The guard expands the glob and checks each
match as well as the pattern. A glob it cannot expand is refused, because it
cannot say where the glob points.

## `--block-path` and `--block-file`

Both flags are deprecated. They are still accepted, and they still subtract:
a blocked path inside an allowed root stays unreachable. The server logs one
deprecation warning at startup when either flag is used. A later release refuses
the flags.

A small built-in block set (`~/.ssh/`, `~/.aws/`, `/etc/shadow` and similar) also
subtracts, for the case where an operator allows a root wide enough to contain
one of them.
