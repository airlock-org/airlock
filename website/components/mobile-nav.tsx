"use client";

import { useRef } from "react";
import Link from "next/link";
import { Menu, ArrowUpRight } from "lucide-react";

export function MobileNav() {
  const menu = useRef<HTMLDetailsElement>(null);

  function close() {
    if (!menu.current) return;
    menu.current.open = false;
    menu.current.querySelector("summary")?.focus();
  }

  return (
    <details
      className="mobile-menu"
      ref={menu}
      onKeyDown={event => { if (event.key === "Escape") close(); }}
    >
      <summary aria-label="Open navigation"><Menu size={21} aria-hidden="true" /></summary>
      <nav aria-label="Mobile navigation">
        <a href="#main" onClick={close}>Home</a>
        <a href="#architecture" onClick={close}>Architecture</a>
        <Link href="/docs/" onClick={close}>Documentation <ArrowUpRight size={14} aria-hidden="true" /></Link>
        <Link href="/docs/status/" onClick={close}>Validation status</Link>
        <Link href="/docs/getting-started/" onClick={close}>Getting started <ArrowUpRight size={14} aria-hidden="true" /></Link>
      </nav>
    </details>
  );
}
