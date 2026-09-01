class Envforge < Formula
  desc "Open-source AI-safe environment manager and secret provider CLI"
  homepage "https://github.com/emreerinc/envforge"
  url "https://github.com/emreerinc/envforge/archive/refs/tags/v1.0.2.tar.gz"
  sha256 "de6451f53c87c95b4a543829d1c24b76670d69304a52a972caa498774035ef73"
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