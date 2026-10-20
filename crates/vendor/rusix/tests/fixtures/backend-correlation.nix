# Foreign functions are deliberately outside the compiler's known package scope.
{
  package = pkgs: attrs: pkgs.stdenv.mkDerivation attrs;
}
