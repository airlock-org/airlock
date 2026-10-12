import Link from "next/link";

export function BrandLabel() {
  return (
    <span className="airlock-brand">
      <img className="brand-logo" src="/logo.svg" width="40" height="40" alt="" aria-hidden="true" />
      <span>AIRLOCK<span className="brand-os">OS</span></span>
    </span>
  );
}

export function Brand() {
  return <Link href="/" aria-label="Airlock home"><BrandLabel /></Link>;
}
