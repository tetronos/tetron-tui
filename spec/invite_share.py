'''
Save an invite to a file and share it on the LAN from the invite overlay
(USER 2026-11-04, 2026-11-19). Sharing runs core contrib
tetron-invite-serve.py (core INVITE-SERVE-001) as a child process; the TUI
serves nothing itself.
'''

from .err import Req


class InviteSave(Req):
    """REQUIREMENT-ID: TUI-INVITE-001

    `w` on the invite overlay saves the invite to
    $HOME/tetron-invite-<network>-<id>.txt, mode 0600, never overwriting an
    existing file, for `tetron join --invite-file`. The overlay lists its keys
    (y copy, w save, s share, Esc close), and shows them as clickable
    buttons (TUI-SERVE-001); a click anywhere else on the card closes it.
    """


class InviteShare(Req):
    """REQUIREMENT-ID: TUI-SERVE-001

    The invite overlay has a row of clickable buttons, each showing its key:
    `y copy`, `w save`, `s share on LAN` (`s stop sharing` while sharing) and
    `Esc close`, with the line "buttons or keys: ..." under them. A button
    does the same as its key. `s` (or its button) shares the shown invite
    over HTTP on the LAN:

    - the TUI runs `python3 -I SCRIPT NETWORK --invite-stdin` as a hidden
      child process (no new window, no terminal), writes the invite and a
      newline to its stdin, then closes stdin, so the invite is never in
      argv or ps;
    - SCRIPT is the first of: $TETRON_INVITE_SERVE, `tetron-invite-serve.py`
      on $PATH, /usr/share/tetron/tetron-invite-serve.py,
      /usr/local/share/tetron/tetron-invite-serve.py. If none exists the toast
      names the file and where it was looked for, and the overlay is unchanged;
    - the first stdout line is the URL (10 s limit); the overlay then shows
      the URL in large type and its QR code in place of the invite QR, and
      the line "sharing on the LAN, Esc stops";
    - if the child ends before printing the URL, the toast shows its stderr
      text; the invite is never part of a message;
    - when the child ends later the overlay goes back to the invite QR and a
      toast names the reason: exit 3 "too many wrong requests", any other
      "stopped". The script itself ends after 5 wrong paths or 1 hour idle;
    - Esc or q (closing the overlay), minting another invite and quitting the
      TUI kill the child. `s` or its button while sharing stops it;
    - the child is killed when the TUI exits for any reason (kill on drop).
    """


class DebDependsPython3(Req):
    """REQUIREMENT-ID: DEB-008

    packaging/deb.toml declares Depends python3 (the invite share child).
    """
