'''
tetron-tui main spec
'''

from libspec import Spec
from . import packaging, constraints, selected_peer, join_input, invite_share, admin_buttons

class MainSpec(Spec):
    def modules(self):
        return [packaging, constraints, selected_peer, join_input, invite_share, admin_buttons]
