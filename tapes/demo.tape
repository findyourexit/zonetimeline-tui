# Hero demo GIF for the top of the README (-> assets/demo.gif).
#
# A guided tour: the live timeline ribbons (cursor sweep + At-cursor inspector),
# a hop to the world map (day/night terminator, per-zone markers), then back.
#
# Regenerate from the repo root:
#   cargo build --release && vhs tapes/demo.tape
# Requires vhs, ttyd and ffmpeg on PATH. Zones live in tapes/demo.toml and the
# launch runs inside the hidden setup block, so the GIF opens on the rendered
# TUI. The anchor is left live, so the view is centred on "now" (v marker).

Output assets/demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 1100
Set Height 640
Set Padding 15
Set Theme "Catppuccin Mocha"
Set TypingSpeed 55ms
Set Framerate 24

# Set up the environment and launch ztl inside the hidden block so the GIF
# starts on the rendered TUI, not the shell command. (Run vhs from the repo
# root; NO_COLOR in the parent shell would otherwise force monochrome.)
Hide
Type `export PATH="$PWD/target/release:$PATH"`
Enter
Type "unset NO_COLOR; export COLORTERM=truecolor"
Enter
Type "clear"
Enter
Type "ztl -c tapes/demo.toml"
Enter
Sleep 2s
Show

# Open on the rendered timeline for a beat before interacting.
Sleep 1.5s

# Sweep the 30-minute cursor across the day; the At-cursor panel tracks it.
Right@220ms 12
Sleep 1s
Left@220ms 12
Sleep 1s

# Jump the cursor to "now" (also marked by the v caret).
Type "n"
Sleep 1.2s

# Walk the zone focus down the stack and back up.
Down@320ms 3
Sleep 700ms
Up@320ms 3
Sleep 1.2s

# Swap to the world map — the striking day/night globe.
Type "m"
Sleep 1.8s

# Walk the focus across the globe: the highlight hops between cities and the
# status line names each zone's local time and availability.
Down@1000ms 4
Sleep 1.4s

# Sweep the cursor forward: the night hemisphere drifts west and every local
# time advances in step.
Right@800ms 5
Sleep 1.4s

# Snap back to now.
Type "n"
Sleep 1.8s

# Return to the timeline to close the loop.
Type "m"
Sleep 1.4s

# Quit inside a hidden block so the exit-to-shell frame is never captured
# (otherwise it flashes the command line when the GIF loops).
Hide
Type "q"
Sleep 400ms
