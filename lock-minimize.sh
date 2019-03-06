#!/usr/bin/bash -ve

cargo -Z minimal-versions generate-lockfile

# Issue: https://github.com/rust-random/getrandom/pull/112
# (merged, awaiting getrandom 0.1.13 release, rand release?)
#
# cfg-if v0.1.0
# ├── getrandom v0.1.7
# │   ├── rand v0.7.0
# │   │   └── tempfile v3.1.0
# │   │       [dev-dependencies]
# │   │       └── olio v1.2.0 (/home/david/src/olio)
# │   │   [dev-dependencies]
# │   │   └── olio v1.2.0 (/home/david/src/olio) (*)
# │   └── rand_core v0.5.0
# │       ├── rand v0.7.0 (*)
# │       ├── rand_chacha v0.2.0
# │       │   └── rand v0.7.0 (*)
# │       └── rand_hc v0.2.0
# │           [dev-dependencies]
# │           └── rand v0.7.0 (*)
# └── tempfile v3.1.0 (*)
cargo update -p cfg-if --precise 0.1.2

cargo update -p winapi:0.2.0 --precise 0.2.8
