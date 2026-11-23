from libspec import Requirement


class DebReleaseAsset(Requirement):
    """REQUIREMENT-ID: DEB-001

    Each release publishes tetron-tui_<version>-1_<arch>.deb for amd64 and
    arm64, with a .sha256 file, next to the existing raw binaries. The raw
    binary assets and their names are unchanged (the suite installer and
    tetron-webui depend on them). The .deb is built from the exact binary
    that the same job uploads as the raw asset.
    """


class DebBuiltByPinnedAction(Requirement):
    """REQUIREMENT-ID: DEB-002

    The .deb is built by the build-deb action of tetron-packaging pinned to a
    full 40 character commit SHA, never a branch or tag. The install test of
    the action (clean debian:trixie container) passes before the release
    upload step runs.
    """


class DebDescription(Requirement):
    """REQUIREMENT-ID: DEB-003

    packaging/deb.toml describes the package: name tetron-tui, installs
    /usr/bin/tetron-tui mode 0755, license MPL-2.0, no maintainer scripts, no
    unit file. Runtime Depends are computed from the binary by mkdeb. The
    tetron daemon is a Recommends, not a Depends, because a host may run the
    daemon from a binary install and apt must not try to fetch a tetron
    package that no configured repository provides. The Recommends follows
    the suite version policy: tetron-tui X.Y.z targets tetron core X.Y, so it
    is tetron (>= X.Y), tetron (<< X.(Y+1)), expanded from the package
    version by mkdeb.
    """


class DebDoesNotChangeBinaryInstall(Requirement):
    """REQUIREMENT-ID: DEB-004

    Adding the .deb does not change how the binary install works: the raw
    asset names, the .sha256 files, the version and tag convention, and
    cargo build are unchanged.
    """


class DebMaintainerFromVariable(Requirement):
    """REQUIREMENT-ID: DEB-006

    No maintainer identity is hardcoded in the repository. The workflow
    passes the repository variable DEB_MAINTAINER to the build-deb action as
    the maintainer input. If the variable is unset the build fails (mkdeb
    requires a maintainer), so a package is never published with an invented
    identity.
    """


class BumpProtoWorkflow(Requirement):
    """REQUIREMENT-ID: DEB-009

    `.github/workflows/bump-proto.yml` runs on the `repository_dispatch`
    event `tetron-release` (sent by tetron core's release workflow with the
    tag) and on manual dispatch with a tag: `contrib/bump-proto.sh` moves the
    `tetron-proto` tag, then `contrib/check-suite-deps.sh`, `cargo test` and
    `cargo clippy --all-targets` run, and a pull request is opened with the
    change and the result. When the tag has tetron-tui's minor and every
    check passed, the pull request merges itself (rebase and merge); a tag
    of another minor, or any failure, leaves it open for review. It never
    pushes to main directly. The release workflow runs
    `contrib/check-suite-deps.sh` before it builds.
    """
