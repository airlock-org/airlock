import Link from "next/link";

export function BrandLabel() {
  return (
    <span className="airlock-brand">
      <svg viewBox="0 0 32 32" width="30" height="30" fill="none" aria-hidden="true">
        <rect x="3" y="3" width="26" height="26" rx="5" stroke="currentColor" strokeWidth="2" />
        <path d="M12 9h11v14H12M9 12v8M17 14v4" stroke="currentColor" strokeWidth="2" />
      </svg>
      <span>AIRLOCK</span>
    </span>
  );
}

export function Brand() {
  return <Link href="/" aria-label="Airlock home"><BrandLabel /></Link>;
}
