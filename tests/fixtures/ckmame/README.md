# ckmame test datafiles

The datafiles in this directory and in `../ckmame-invalid/` are taken from the
regression suite of [ckmame](https://github.com/nih-at/ckmame), an independent
ROM manager that reads the same ClrMamePro syntax. They are kept here so that
`datary` is tested against files it did not author — every ClrMamePro bug found
in this crate so far came from reading ckmame or its data, never from fixtures
written against our own assumptions.

Each covers a construct that is easy to get wrong:

| File | Covers |
| --- | --- |
| `mamedb-size-hex.dat` | `size "0x10"` — a hexadecimal size |
| `mamedb-size-empty.dat` | a `rom` with no `size` key at all |
| `mamedb-xml-quoting.dat` | `<`, `&`, `>` in values, which must survive conversion to XML |
| `mamedb-deadbeefish.dat` | `0x`-prefixed checksums |
| `mamedb-duplicate-rom-name.dat` | two ROMs sharing a name within one game |
| `mamedb-lost-parent.dat` | a `cloneof` naming a game that is not in the file |
| `mamedb-merge-parent.dat` | `merge` resolving through a parent set |
| `../ckmame-invalid/broken-sha1.dat.bad` | a SHA-1 of the wrong width |
| `../ckmame-invalid/missing-size.dat.bad` | `size` with its value omitted |
| `../ckmame-invalid/unbalanced-braces.dat.bad` | `game ( (` |

The invalid ones use a `.dat.bad` extension so that the `*.dat` fixture globs,
which require every match to parse, do not pick them up.

## Copyright

    Copyright (C) 1999-2024 Dieter Baron and Thomas Klausner
    The authors can be contacted at <ckmame@nih.at>

    Redistribution and use in source and binary forms, with or without
    modification, are permitted provided that the following conditions
    are met:
    1. Redistributions of source code must retain the above copyright
       notice, this list of conditions and the following disclaimer.
    2. Redistributions in binary form must reproduce the above copyright
       notice, this list of conditions and the following disclaimer in
       the documentation and/or other materials provided with the
       distribution.
    3. The name of the author may not be used to endorse or promote
       products derived from this software without specific prior
       written permission.

    THIS SOFTWARE IS PROVIDED BY THE AUTHORS ``AS IS'' AND ANY EXPRESS
    OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
    WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
    ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR ANY
    DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
    DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE
    GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
    INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
    WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
    NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
    SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

This is BSD-3-Clause, which permits redistribution provided the notice above is
retained. It is compatible with this crate's Apache-2.0 licence. The files are
unmodified; only their names differ, and only for the invalid ones.
