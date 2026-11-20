---
title: Archive processing
description: rikka-archive reads and writes common formats with streaming batches, AES, cancellation and traversal defence.
about: crates/foundation/rikka-archive
tags: [archive, security]
---

- `Archive` opens paths, readers or bytes for listing, selective reads, copies and extraction. Extraction options: overwrite, strip, metadata, symlinks, password, cancel, filter, progress.
- `ArchiveWriter` adds bytes, readers, dirs, symlinks and paths. It needs a consuming `finish()`.
- Formats: ZIP, 7z, tar, tar.{gz,bz2,xz,zst} and single streams. ZIP and 7z support AES-256. Content detection probes up to 4 MiB of decompressed data for tar.
- Reads and writes share one path policy (no drive prefixes, roots, traversal, unsafe symlinks or special entries), so it never writes what it would refuse. Extraction creates nothing through a symlink below the destination, and a link target may only climb first (`../..`) and then descend, so chains of links cannot leave it.
- `read_each` streams many entries in one pass. Ascending batches avoid re-decompressing solid blocks.
- Metadata failures keep the bytes and go into a bounded `MetadataReport`. Non-UTF-8 names are recovered by encoding detection.
- Blocking on the caller's thread, with cancellation and no pool.
