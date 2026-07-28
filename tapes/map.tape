# "The World Map View" GIF (-> assets/map.gif): the configured zones plotted on
# a braille Mercator globe with a live day/night terminator. Walking the zone
# focus hops the highlight between cities; sweeping the cursor shifts day/night
# and every local time.
#
# The frame is wide enough that the aspect-correct world tiles horizontally to
# fill the surplus width (the globe wraps like a cylinder); marker dots repeat
# across the copies while each label is drawn once.
#
# Regenerate from the repo root:
#   cargo build --release && vhs tapes/map.tape
# Requires vhs, ttyd and ffmpeg on PATH. Zones live in tapes/demo.toml.

Output assets/map.gif

Set Shell "bash"
Set FontSize 16
Set Width 1100
Set Height 620
Set Padding 15
Set Theme "Catppuccin Mocha"
Set TypingSpeed 55ms
Set Framerate 24

# Launch inside the hidden block so the GIF opens on the rendered map. (Force
# truecolour; a parent-shell NO_COLOR would otherwise flatten the shading.)
Hide
Type `export PATH="$PWD/target/release:$PATH"`
Enter
Type "unset NO_COLOR; export COLORTERM=truecolor"
Enter
Type "clear"
Enter
Type "ztl --map -c tapes/demo.toml"
Enter
Sleep 2s
Show

# Open on the rendered globe for a beat.
Sleep 1.6s

# Walk the focus across the globe: the ◉ highlight hops between cities and the
# status line names each zone's local time and availability.
Down@1200ms 4
Sleep 1.4s

# Sweep the cursor forward: the night hemisphere drifts west and every local
# time advances in step.
Right@900ms 4
Sleep 1.6s

# Snap back to now.
Type "n"
Sleep 2.2s

# Quit hidden so the exit-to-shell frame is never captured.
Hide
Type "q"
Sleep 400ms
