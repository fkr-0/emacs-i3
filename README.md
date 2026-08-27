# Emacs i3 unified window management

Inspired by https://sqrtminusone.xyz/posts/2021-10-04-emacs-i3/

## Usage

Call this program in your i3 config:

```
bindsym Mod4+h exec emacs-i3 focus left
bindsym Mod4+j exec emacs-i3 focus down
bindsym Mod4+k exec emacs-i3 focus up
bindsym Mod4+l exec emacs-i3 focus right
```

Any i3-style command passed after `emacs-i3` is first offered to Emacs when the
focused window is an Emacs frame.  If Emacs cannot handle it, or if the focused
window is not Emacs, the command is forwarded to i3 unchanged.

Supported Emacs-side commands include:

```
emacs-i3 focus left
emacs-i3 move right
emacs-i3 resize grow width 10 px
emacs-i3 layout toggle split
emacs-i3 split h
emacs-i3 kill
```

## Local install

```
cargo build --release
ln -sfn "$PWD/target/release/emacs-i3" "$HOME/.local/bin/emacs-i3"
```
