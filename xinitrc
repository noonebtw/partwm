#!/bin/sh

/usr/bin/xset b off
/usr/bin/xsetroot -solid darkslategrey
/usr/bin/feh --bg-fill "/mnt/storage/rust/wm/starship.jpg"
export RUST_BACKTRACE=1
exec /mnt/storage/rust/wm/target/debug/wm >& /home/user/.local/portlights.log
