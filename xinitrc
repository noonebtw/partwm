#!/bin/sh

/usr/bin/xset b off
/usr/bin/xsetroot -solid darkslategrey
/usr/bin/feh --bg-fill "/mnt/storage/rust/wm/starship.jpg"
exec /mnt/storage/rust/wm/target/release/wm
