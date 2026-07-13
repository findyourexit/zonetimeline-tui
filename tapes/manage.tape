# Zone-management GIF (-> assets/manage.gif): add, reorder, edit a work window,
# and remove a zone through the interactive modals.
#
# Regenerate from the repo root:
#   cargo build --release && vhs tapes/manage.tape

Output assets/manage.gif

Set Shell "bash"
Set FontSize 16
Set Width 1100
Set Height 620
Set Padding 15
Set Theme "Catppuccin Mocha"
Set TypingSpeed 90ms
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
Type "ztl -c tapes/manage.toml"
Enter
Sleep 2s
Show

# Open on the rendered timeline for a beat before managing zones.
Sleep 1.2s

# Add a zone: 'a' opens the picker, type to filter, Enter adds the match.
Type "a"
Sleep 1s
Type "Tokyo"
Sleep 1.5s
Enter
Sleep 1.8s

# Walk the zone focus down to the newly added zone (clamps on the last row).
Down@280ms 5
Sleep 1.3s

# Edit its work window: arrows scroll, Tab switches start/end, Enter commits.
Type "e"
Sleep 1.5s
Down@250ms 3
Sleep 800ms
Tab
Sleep 800ms
Up@250ms 2
Sleep 900ms
Enter
Sleep 1.6s

# Remove the focused zone with x.
Type "x"
Sleep 1.8s

# Quit hidden so the exit-to-shell frame is never captured (it would otherwise
# flash the command line when the GIF loops).
Hide
Type "q"
Sleep 400ms
