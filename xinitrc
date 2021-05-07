#!/bin/sh

/usr/bin/xset b off
/usr/bin/xsetroot -solid darkslategrey
/usr/bin/feh --bg-fill "/mnt/storage/rust/wm/starship.jpg"
xset r rate 250 30
export RUST_BACKTRACE=1
exec /mnt/storage/rust/wm/target/release/wm >& /home/user/.local/portlights.log
