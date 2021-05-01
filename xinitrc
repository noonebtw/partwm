#!/bin/sh

/usr/bin/xset b off
/usr/bin/xsetroot -solid darkslategrey
/usr/bin/feh --bg-fill "./starship.jpg"
export RUST_BACKTRACE=1
exec ./target/debug/wm >& &HOME/.portlights.log
