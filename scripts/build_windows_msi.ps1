# WITHDRAWN during Phase Zero (P0-FINAL-GATE-CLOSURE, dossier item J4).
#
# This script built nexus-cli with cargo and an MSI from
# packaging/windows/nexus-os.wxs. nexus-cli is withdrawn: it only prints a
# withdrawal message. The desktop app's Windows installers are built by the
# release workflow, not by this script.
#
# Running this file fails immediately, on purpose. It reads no parameter,
# builds nothing and packages nothing.
[Console]::Error.WriteLine("build_windows_msi.ps1: withdrawn during Phase Zero; nothing is built or packaged")
exit 1
