# Unnamed WM

This Project is a x11 tiling window manager written in Rust and losely based on / inspired by suckless' [dwm](https://dwm.suckless.org/).

It has 2 `stacks` like dwm; one master stack on the left side that will always be populated if there is any windows on the screen, and a secondary `aux` stack to which new windows are automatically added.
You can push windows around from the `master` stack to the `aux` stack and the other way around with `M-m`.

You can resize any window by pressing `M-mouse-2` and moving the mouse and move them the same way with `M-mouse-1`. Moving and resizing a window "unstacks" it and it becomes a freely floating window that will stay ontop of the tiled windows. Pressing `M-mouse-3` will "re-tile" a floating window.

Both `M-S-T` and `M-S-RET` will spawn an instance of `xterm`, `M-q` will kill the focused window and `M-S-q` will kill the window manager.

One big difference from dwm is the way I handle virtual screens, although this is mostly a placeholder mechanic that I will most likely change in the future. Currently I have 3 (or more) virtual screens in a list that can be rotated with `M-left` and `M-right`.

Unnamed WM also has optional gaps :^)
![Unnamed WM in a VM](/vm-ss.png)
