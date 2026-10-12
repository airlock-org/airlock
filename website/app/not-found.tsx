import Link from "next/link";

export default function NotFound() {
  return (
    <main className="airlock-missing">
      <p className="eyebrow">404 / PAGE MISSING</p>
      <h1>This route has no page.</h1>
      <p>Browse the documentation or return to the project overview.</p>
      <div className="button-row">
        <Link href="/docs/" className="site-button primary">Documentation</Link>
        <Link href="/" className="site-button secondary">Home</Link>
      </div>
    </main>
  );
}
