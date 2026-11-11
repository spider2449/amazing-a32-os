# Kernel source case conflicts

1. Preserve the existing Windows-hosted source as a backup.
2. Check out the pinned source on WSL's Linux filesystem and retain the expected path through a symlink.
3. Reject case-insensitive source directories before checkout in the preparation script.
4. Apply the existing patches and verify the previously failing Netfilter build.

Completed: source and output now live under
`/home/morefunfun/.cache/a32-kernel-casefix/`, with the original workspace paths
preserved as WSL symlinks. Original directories remain as `kernel-4.9-case-backup`
and `build-case-backup`. Source preparation and the complete `net/netfilter/`
target passed, including distinct `xt_HL.o` and `xt_hl.o`. Preparation/build now
reject case-insensitive filesystems. Repeated patch detection tolerates adjacent
patch context changes. Shell syntax and whitespace checks passed.
