# "The Timeline View" GIF (-> assets/timeline.gif): the 30-minute cursor and
# the live At-cursor inspector, plus the v now marker and best-window summary.
#
# Regenerate from the repo root:
#   cargo build --release && vhs tapes/timeline.tape

Output assets/timeline.gif

Set Shell "bash"
Set FontSize 16
Set Width 1100
Set Height 620
Set Padding 15
Set Theme "Catppuccin Mocha"
Set TypingSpeed 55ms
Set Framerate 24

# Launch inside the hidden block so the GIF opens on the rendered TUI. (Force
# truecolour; a parent-shell NO_COLOR would otherwise flatten the ribbons.)
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

# Open on the rendered timeline for a beat before stepping the cursor.
Sleep 1.3s

# Step the cursor slowly so the At-cursor inspector is readable.
Right 1
Sleep 1.3s
Right 1
Sleep 1.3s
Right 1
Sleep 1.3s
Right 1
Sleep 1.6s

# Move the highlighted zone down the stack.
Down 1
Sleep 1.3s
Down 1
Sleep 1.6s

# Snap the cursor back to now (the v marker).
Type "n"
Sleep 2.5s

# Quit hidden so the exit-to-shell frame is never captured (it would otherwise
# flash the command line when the GIF loops).
Hide
Type "q"
Sleep 400ms
