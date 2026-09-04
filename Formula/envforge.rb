class Envforge < Formula
  desc "AI-safe environment manager and secret provider CLI"
  homepage "https://envforge.tech"
  url "https://github.com/emreerinc/envforge/archive/refs/tags/v1.0.3.tar.gz"
  sha256 "dbf88daeac97e7670983a08094d41af38b398d1a89eb38c3b3e560de3d64b4a0"
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