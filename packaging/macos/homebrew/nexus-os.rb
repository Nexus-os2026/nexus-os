class NexusOs < Formula
  desc "Governed agent runtime with auditable policy controls"
  homepage "https://github.com/nexai-lang/nexus-os"
  url "https://github.com/nexai-lang/nexus-os/archive/refs/tags/v1.0.0.tar.gz"
  sha256 "REPLACE_WITH_RELEASE_TARBALL_SHA256"
  license "TBD"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release", "-p", "nexus-cli"
    bin.install "target/release/nexus-cli"
    (prefix/"com.nexusos.agent.plist").write File.read("packaging/macos/com.nexusos.agent.plist")
  end

  service do
    run [opt_bin/"nexus-cli"]
    keep_alive true
    working_dir var/"nexus-os"
    log_path var/"log/nexus-os.log"
    error_log_path var/"log/nexus-os.err.log"
  end

  test do
    assert_match "nexus", shell_output("#{bin}/nexus-cli --help")
  end
end
