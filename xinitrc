#!/bin/sh

xset b off
xsetroot -solid darkslategrey
feh --bg-fill "/mnt/storage/rust/wm/starship.jpg"
xset r rate 250 30
export RUST_BACKTRACE=1
exec /mnt/storage/code/rust/wm/target/release/wm 2>&1 /home/user/.local/portlights.log
