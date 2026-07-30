class Envforge < Formula
  desc "Open-source AI-safe environment manager and secret provider CLI"
  homepage "https://github.com/emreerinc/envforge"
  url "https://github.com/emreerinc/envforge/archive/refs/tags/v1.0.1.tar.gz"
  sha256 "42d072694f21850ab4b80c895a3ff5b80dc7055e31c039ee17b72d8fcdd0f94f"
  license "Elastic-2.0"
  head "https://github.com/emreerinc/envforge.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args

    # Generate and install shell completions
    generate_completions_from_executable(bin/"envforge", "completions", shells: [:bash, :zsh, :fish])
  end

  test do
    assert_match "envforge", shell_output("#{bin}/envforge --version")
  end
end
