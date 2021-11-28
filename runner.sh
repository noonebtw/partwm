#!/bin/sh

# binary supplied by cargo
bin=$1

# write temporary xinitrc
printf "exec $bin\n" > xinitrc.tmp

# call xinit
XEPHYR_BIN=$(which Xephyr)

[ -z "$XEPHYR_BIN" ] || exec xinit ./xinitrc.tmp -- $XEPHYR_BIN :100 -ac -screen 800x600
