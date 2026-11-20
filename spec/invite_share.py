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


class InviteCardKeepsOpen(Req):
    """REQUIREMENT-ID: TUI-INVITE-003

    Bug fix, priority (USER 2026-11-19). The invite card is a window the
    user must be able to read, copy from and scroll:

    - a click on the card itself does nothing; the card used to vanish, so
      the invite was lost. It closes only by Esc, Enter, q, the `Esc close`
      button, or a click outside the card;
    - copy (`y`, `c`, the `y copy` button) sends the invite to the terminal
      clipboard with OSC 52 and also to the first installed system tool of
      wl-copy, xclip, xsel; the toast names what was used and says that
      `m` then a drag selects text by hand when nothing pastes;
    - `m` (button `m select text`, `m mouse on` while off) toggles mouse
      capture so the terminal can select the code text natively;
    - the card is at least 66 columns wide when the window allows, so all
      buttons show their labels; when it does not fit, buttons show only
      their keys;
    - when the window is too small for the whole card, the content (code,
      id, QR) scrolls above two fixed rows (buttons and a hint) with Up,
      Down, j, k, PageUp, PageDown, Home or the mouse wheel, and the hint
      says so; the QR can always be reached in full. The scroll position
      resets when a new invite opens.
    """


class InvitePickNetwork(Req):
    """REQUIREMENT-ID: TUI-INVITE-002

    `i` and the `invite` toolbar button mint an invite for a network chosen
    by the user (USER 2026-11-19). With one network the invite is minted
    directly. With several, a picker "Invite for which network?" opens first,
    listing each network (number, name, role, subnet) with the selected one
    highlighted as the default. A number key, Enter on the highlighted row
    or a click on a row picks it, makes it the selected network and mints the
    invite; Up/Down (or k/j) move; Esc, q or a click outside cancels without
    minting anything. The invite overlay title names the network.
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
