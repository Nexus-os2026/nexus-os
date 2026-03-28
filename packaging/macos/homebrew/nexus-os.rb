class NexusOs < Formula
  desc "Governed agent runtime with auditable policy controls"
  homepage "https://gitlab.com/nexaiceo/nexus-os"
  url "https://gitlab.com/nexaiceo/nexus-os/-/archive/v10.3.0/nexus-os-v10.3.0.tar.gz"
  sha256 "" # Computed from release tarball after tag is pushed
  license "MIT"

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
