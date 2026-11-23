from libspec import Constraint


class DebTomlValid(Constraint):
    """CONSTRAINT-ID: CON-D01

    packaging/deb.toml parses, installs the binary to usr/bin/tetron-tui
    with mode 0755, and has no scripts section.
    ENFORCEMENT (reconcile.py): deb_toml.valid is true.
    """
    constraint_id = "CON-D01"
    enforcement_logic = "{{ deb_toml.valid }}"


class ActionPinnedBySha(Constraint):
    """CONSTRAINT-ID: CON-D02

    Every use of tetronos/tetron-packaging in .github/workflows is
    pinned to a full 40 hex character commit SHA.
    ENFORCEMENT (reconcile.py): workflow.unpinned equals 0 and
    workflow.uses_action is true.
    """
    constraint_id = "CON-D02"
    enforcement_logic = "{{ workflow.unpinned == 0 and workflow.uses_action }}"


class RawAssetsUnchanged(Constraint):
    """CONSTRAINT-ID: CON-D03

    The release workflow still builds and uploads the raw binary assets
    tetron-tui-linux-x86_64, tetron-tui-linux-aarch64, and their .sha256
    files.
    ENFORCEMENT (reconcile.py): workflow.raw_assets is true.
    """
    constraint_id = "CON-D03"
    enforcement_logic = "{{ workflow.raw_assets }}"


class SuiteGitDepsPinned(Constraint):
    """CONSTRAINT-ID: CON-D04

    Every git dependency in `Cargo.toml` on a `github.com/tetronos/`
    repository names a release tag, and the `tetron-proto` tag has the same
    minor as tetron-tui's own version. Checked by
    `contrib/check-suite-deps.sh`, the same script in every suite client.
    ENFORCEMENT (reconcile.py): suite_deps.ok is true.
    """
    constraint_id = "CON-D04"
    enforcement_logic = "{{ suite_deps.ok }}"
