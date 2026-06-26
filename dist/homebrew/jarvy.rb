# Homebrew formula for Jarvy
# To update: Replace VERSION_PLACEHOLDER and SHA256 placeholders with actual values
# Tap repository: https://github.com/Cliftonz/homebrew-tap

class Jarvy < Formula
  desc "Fast, cross-platform CLI that installs and manages developer tools"
  homepage "https://github.com/Cliftonz/jarvy"
  version "VERSION_PLACEHOLDER"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_intel do
      url "https://github.com/Cliftonz/jarvy/releases/download/v#{version}/jarvy-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "SHA256_PLACEHOLDER_MACOS_X86"
    end

    on_arm do
      url "https://github.com/Cliftonz/jarvy/releases/download/v#{version}/jarvy-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "SHA256_PLACEHOLDER_MACOS_ARM"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Cliftonz/jarvy/releases/download/v#{version}/jarvy-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "SHA256_PLACEHOLDER_LINUX_X86"
    end

    on_arm do
      url "https://github.com/Cliftonz/jarvy/releases/download/v#{version}/jarvy-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "SHA256_PLACEHOLDER_LINUX_ARM"
    end
  end

  def install
    bin.install "jarvy"

    # Generate shell completions if the binary supports it
    # Uncomment when shell completions are implemented:
    # generate_completions_from_executable(bin/"jarvy", "completions")
  end

  def caveats
    <<~EOS
      To get started with Jarvy, create a jarvy.toml in your project:

        jarvy configure

      Then provision your development environment:

        jarvy setup

      For more information, see: https://github.com/Cliftonz/jarvy
    EOS
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/jarvy --version")

    # Test that basic command works
    assert_match "Usage:", shell_output("#{bin}/jarvy --help")
  end
end
