'''
Admin tab actions are mouse clickable (USER 2026-11-27: make kick easier).
'''

from .err import Req


class AdminActionButtons(Req):
    """REQUIREMENT-ID: TUI-ADMIN-BTN-001

    The Admin tab shows its actions (a grant admin, K kick, X nuke, L leave)
    as buttons. A left click on a button does the same as its key: Kick
    opens the existing confirmation naming the peer and network, and y then
    sends the kick. Buttons for coordinator-only actions are dimmed when this
    node does not hold the network key; a click then shows "admins only".
    Only a network key holder (coordinator) can kick; the daemon enforces it.
    """


class ConfirmButtons(Req):
    """REQUIREMENT-ID: TUI-CONFIRM-BTN-001

    Every confirm dialog has two clickable buttons: a red yes button named
    after the action ("y kick", "y nuke", "y leave", "y grant admin",
    "y stop daemon", "y fetch") and "n cancel". Cancel is the default: only
    a click on the yes button or the y key runs the action; any other click
    or key closes the dialog without acting.
    """
